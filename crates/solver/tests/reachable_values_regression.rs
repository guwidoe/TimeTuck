use std::sync::Arc;
use timetuck::*;
#[test]
fn all_small_domains_match_direct_membership_and_optional_transition_queries() {
    for mask in 0..512usize {
        for optional in 0..8usize {
            let ranges = (0..3)
                .map(|entity| {
                    (
                        entity,
                        (0..3)
                            .filter(|v| mask & (1 << (entity * 3 + v)) != 0)
                            .collect::<Vec<_>>(),
                        optional & (1 << entity) != 0,
                    )
                })
                .collect::<Vec<_>>();
            let index = ReachableValues::new(ranges.clone()).unwrap();
            for value in 0..=3 {
                assert_eq!(
                    index.entities(&value),
                    ranges
                        .iter()
                        .filter(|(_, values, _)| values.contains(&value))
                        .map(|(e, _, _)| *e)
                        .collect::<Vec<_>>()
                );
                let expected = (0..3)
                    .filter(|target| {
                        *target != value
                            && ranges.iter().any(|(_, values, _)| {
                                values.contains(&value) && values.contains(target)
                            })
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    index
                        .co_values(&value)
                        .into_iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    expected
                );
            }
            let choices = [None, Some(0), Some(1), Some(2), Some(3)];
            for origin in choices {
                for target in choices {
                    let expected = origin != target
                        && ranges.iter().any(|(_, values, allow)| {
                            origin.map_or(*allow, |v| values.contains(&v))
                                && target.map_or(*allow, |v| values.contains(&v))
                        });
                    assert_eq!(
                        index.is_value_reachable(origin.as_ref(), target.as_ref()),
                        expected
                    );
                }
            }
        }
    }
}
#[test]
fn canonical_ranges_keep_every_entity_and_views_do_not_mutate_snapshots() {
    let index = ReachableValues::new(vec![
        ("B", vec![3, 1, 2, 2], false),
        ("A", vec![2, 3, 1], true),
        ("C", vec![3, 4], false),
    ])
    .unwrap();
    let a = index.domain(&"A").unwrap();
    let b = index.domain(&"B").unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(index.entities(&2), ["A", "B"]);
    assert_eq!(
        index
            .co_values_by(&3, |a, b| b.cmp(a))
            .into_iter()
            .copied()
            .collect::<Vec<_>>(),
        [4, 2, 1]
    );
    assert_eq!(
        index.co_values(&3).into_iter().copied().collect::<Vec<_>>(),
        [1, 2, 4]
    );
    let copy = index.clone();
    assert!(Arc::ptr_eq(&a, &copy.domain(&"A").unwrap()));
    drop(index);
    drop(copy);
    assert_eq!(&*a, &[1, 2, 3]);
}
#[test]
fn unknown_empty_and_duplicate_entities_have_explicit_semantics() {
    assert!(matches!(
        ReachableValues::new(vec![(0, vec![1], false), (0, vec![2], true)]),
        Err(ReachabilityError::DuplicateEntity(0))
    ));
    let index = ReachableValues::new(vec![
        (0, vec![], true),
        (1, vec![1, 2], false),
        (2, vec![2, 3], true),
    ])
    .unwrap();
    assert!(index.entity_accepts(&0, None));
    assert!(!index.entity_accepts(&99, None));
    assert!(index.domain(&99).is_none());
    assert_eq!(index.destinations(None), [Some(&2), Some(&3)]);
    assert_eq!(index.destinations(Some(&2)), [None, Some(&1), Some(&3)]);
    assert!(index.destinations(Some(&99)).is_empty());
    assert!(!index.is_value_reachable(Some(&1), None));
    assert!(!index.is_value_reachable(None, None));
}
#[test]
fn adapters_capture_structural_domains_without_erasing_pins_or_tracking_assignments() {
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![2, 3]).allowing_unassigned(),
    ])
    .unwrap();
    let index = basic_reachability(&basic);
    assert_eq!(index.entities(&1), [0]);
    assert!(!index.entity_accepts(&0, None));
    assert!(index.entity_accepts(&1, None));
    let lists = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![2]).with_value_range(vec![2]),
        ],
    )
    .unwrap();
    let required = list_reachability(&lists);
    assert_eq!(required.entities(&1), [0]);
    assert_eq!(required.entities(&2), [0, 1]);
    assert!(!required.entity_accepts(&0, None));
    let optional = list_reachability(&lists.allowing_unassigned());
    assert!(optional.entity_accepts(&0, None));
    assert!(!required.entity_accepts(&0, None));
}
#[test]
fn inverse_domains_supply_basic_change_candidates_in_both_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 2, 3]).with_initial(0),
    ])
    .unwrap();
    let index = basic_reachability(&p);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap() + *v[1].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            let mut moves = vec![];
            for value in index.values() {
                for &variable in index.entities(value) {
                    if !p.variables()[variable].is_pinned()
                        && a.values()[variable].as_ref() != Some(value)
                    {
                        moves.push(Move::Change {
                            variable,
                            value: Some(*value),
                        });
                    }
                }
            }
            moves
        };
        let config = LocalSearchConfig {
            max_steps: 3,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("sum", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(5));
    }
}
#[test]
fn inverse_domains_supply_only_eligible_list_owners_in_both_backends() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![2]).with_value_range(vec![2]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let index = list_reachability(&p);
    let mut n = ListConstraintNetwork::new(3, 2);
    for value in 0..2 {
        n.add_constraint(
            format!("owner{value}"),
            vec![ListScope::Value(value)],
            |facts| {
                let ListFact::Value { element, .. } = &facts[0] else {
                    unreachable!()
                };
                SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
            },
        )
        .unwrap();
    }
    for stream in [false, true] {
        let mut provider = |p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            let mut moves = vec![];
            for value in index.values() {
                let source = a.location(value).unwrap();
                if p.entities()[source.entity].is_pinned()
                    || source.index < p.entities()[source.entity].pinned_prefix()
                {
                    continue;
                }
                for &entity in index.entities(value) {
                    if entity != source.entity && !p.entities()[entity].is_pinned() {
                        moves.push(ListMove::Change {
                            source,
                            destination: ListPosition {
                                entity,
                                index: a.lists()[entity].len(),
                            },
                        });
                    }
                }
            }
            moves
        };
        let config = LocalSearchConfig {
            max_steps: 3,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("owners", sources.values.port(), |v| {
                SimpleScore::new(v.position.unwrap().entity as i64)
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(4));
    }
}

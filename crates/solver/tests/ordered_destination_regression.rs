use std::num::NonZeroUsize;
use timetuck::*;
fn provider() -> SampledAnchoredListChangeProvider<i32> {
    SampledAnchoredListChangeProvider {
        values: vec![1, 2, 3],
        entities: vec![1, 0],
        anchors: vec![3, 1, 2],
        include_unassigned: false,
        include_noops: false,
        seed: 71,
        attempts_per_step: 200,
    }
}
#[test]
fn all_small_queries_match_independent_slot_enumeration() {
    for code in 0..27 {
        for full_pin in [false, true] {
            let mut rows = [vec![], vec![]];
            let mut code = code;
            for v in 1..=3 {
                let owner = code % 3;
                code /= 3;
                if owner < 2 {
                    rows[owner].push(v);
                }
            }
            let p = ListProblem::new(
                vec![1, 2, 3],
                vec![
                    ListEntity::new(rows[0].clone()).with_pinned_prefix(rows[0].len().min(1)),
                    ListEntity::new(rows[1].clone()).with_pinned(full_pin),
                ],
            )
            .unwrap()
            .allowing_unassigned();
            let a = p.initial_assignment();
            let selector =
                ListDestinationSelector::new(&p, &a, &[1, 0, 1], &[3, 1, 2, 1], true).unwrap();
            for mask in 0..8 {
                let group: Vec<_> = (1..=3).filter(|v| mask & (1 << (v - 1)) != 0).collect();
                let mut expected = vec![None];
                for entity in [1, 0, 1] {
                    let meta = &p.entities()[entity];
                    if !meta.is_pinned() {
                        expected.push(Some(ListPosition {
                            entity,
                            index: meta.pinned_prefix(),
                        }));
                    }
                }
                for anchor in [3, 1, 2, 1] {
                    if group.contains(&anchor) {
                        continue;
                    }
                    for (entity, row) in rows.iter().enumerate() {
                        for (index, &v) in row.iter().enumerate() {
                            if v == anchor
                                && !p.entities()[entity].is_pinned()
                                && index >= p.entities()[entity].pinned_prefix()
                            {
                                expected.push(Some(ListPosition {
                                    entity,
                                    index: index + 1,
                                }));
                            }
                        }
                    }
                }
                let result = selector.for_values(&group).unwrap();
                assert_eq!(result.iter().collect::<Vec<_>>(), expected);
                for (i, p) in expected.iter().enumerate() {
                    assert_eq!(result.get(i), Some(*p));
                }
                assert_eq!(result.get(result.len()), None);
                assert_eq!(
                    result.iter().rev().collect::<Vec<_>>(),
                    expected.iter().rev().copied().collect::<Vec<_>>()
                );
            }
        }
    }
}
#[test]
fn range_queries_refresh_exclusions_without_mutating_old_orders() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let selector = ListDestinationSelector::new(&p, &a, &[0, 1], &[3, 1, 2], true).unwrap();
    let query = |group: &[i32]| {
        selector
            .for_values(group)
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
    };
    let first = query(&[1]);
    assert_eq!(
        first,
        vec![
            None,
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        ]
    );
    assert_eq!(
        query(&[3]),
        vec![
            None,
            Some(ListPosition {
                entity: 1,
                index: 0
            })
        ]
    );
    assert_eq!(query(&[1, 3]), vec![None]);
    assert_eq!(
        query(&[2, 2]),
        vec![
            None,
            Some(ListPosition {
                entity: 0,
                index: 0
            }),
            Some(ListPosition {
                entity: 1,
                index: 0
            }),
            Some(ListPosition {
                entity: 1,
                index: 1
            }),
            Some(ListPosition {
                entity: 0,
                index: 1
            })
        ]
    );
    assert_eq!(query(&[1]), first);
    assert!(selector.for_values(&[9]).is_err());
    assert!(ListDestinationSelector::new(&p, &a, &[2], &[], true).is_err());
    assert!(ListDestinationSelector::new(&p, &a, &[], &[9], true).is_err());
    assert!(
        ListDestinationSelector::new(&p, &ListAssignment::new(vec![]), &[], &[], true).is_err()
    );
}
#[test]
fn sampler_replays_filtered_pair_oracle_and_keeps_undo_sound() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2]),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut provider = provider();
    provider.include_unassigned = true;
    let candidates: Vec<_> = provider.select(&p, &a, 4).unwrap().collect();
    let mut rng = SampleRandom::new(75);
    let mut expected = vec![];
    for _ in 0..provider.attempts_per_step {
        let value = provider.values[rng.index(NonZeroUsize::new(3).unwrap())];
        // Independent explicit source-specific destination orders.
        let slots = match value {
            1 => vec![
                None,
                Some(ListPosition {
                    entity: 0,
                    index: 0,
                }),
                Some(ListPosition {
                    entity: 0,
                    index: 2,
                }),
            ],
            2 => vec![
                None,
                Some(ListPosition {
                    entity: 1,
                    index: 0,
                }),
                Some(ListPosition {
                    entity: 0,
                    index: 0,
                }),
                Some(ListPosition {
                    entity: 1,
                    index: 1,
                }),
                Some(ListPosition {
                    entity: 0,
                    index: 1,
                }),
            ],
            3 => vec![
                None,
                Some(ListPosition {
                    entity: 1,
                    index: 0,
                }),
            ],
            _ => unreachable!(),
        };
        let destination = slots[rng.index(NonZeroUsize::new(slots.len()).unwrap())];
        expected.extend(
            list_value_changes(
                &p,
                &a,
                &[value],
                &[destination],
                ListChangePairing::Paired,
                false,
            )
            .unwrap(),
        );
    }
    assert_eq!(candidates, expected);
    assert!(!candidates.is_empty() && candidates.len() < provider.attempts_per_step as usize);
    for candidate in &candidates {
        let mut trial = a.clone();
        drop(p.apply(&mut trial, candidate.movement()).unwrap());
        assert_eq!(trial, a);
    }
    drop(
        provider
            .select(&p, &a, 4)
            .unwrap()
            .take(3)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        provider.select(&p, &a, 4).unwrap().collect::<Vec<_>>(),
        candidates
    );
    provider.seed = 75;
    assert_eq!(
        provider.select(&p, &a, 0).unwrap().collect::<Vec<_>>(),
        candidates
    );
}
#[test]
fn missing_values_can_be_assigned_and_invalid_or_empty_samples_are_bounded() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut provider = provider();
    assert_eq!(provider.select(&p, &a, 0).unwrap().count(), 200);
    provider.attempts_per_step = 0;
    assert_eq!(provider.select(&p, &a, 0).unwrap().count(), 0);
    provider.anchors.push(9);
    assert!(provider.select(&p, &a, 0).is_err());
    let pinned = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 2, 3]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let a = pinned.initial_assignment();
    let mut provider = crate::provider();
    provider.attempts_per_step = u64::MAX;
    assert_eq!(provider.select(&pinned, &a, 0).unwrap().count(), 0);
    let blocked = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![]).with_value_range(vec![]),
            ListEntity::new(vec![]).with_value_range(vec![]),
        ],
    )
    .unwrap();
    let a = blocked.initial_assignment();
    provider.attempts_per_step = 10;
    assert_eq!(provider.select(&blocked, &a, 0).unwrap().count(), 0);
    let optional = blocked.allowing_unassigned();
    let a = optional.initial_assignment();
    provider.include_unassigned = true;
    provider.include_noops = true;
    assert_eq!(provider.select(&optional, &a, 0).unwrap().count(), 10);
}

#[test]
fn provider_constructs_lists_and_relocates_for_both_model_types() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("receiver", vec![ListScope::Entity(1)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let plan = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                assert_scores: true,
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let mut construct = provider();
    let mut improve = provider();
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(0, &mut construct);
    extensions.move_providers.bind(1, &mut improve);
    let result = solve_list_with_extensions(&p, &n, plan, extensions).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    assert!(result.assignment.lists()[0].is_empty());
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![8, 9]).with_initial(8)]).unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 3);
    n.add_constraint("receiver", vec![MixedScope::ListEntity(1)], |f| {
        let MixedFact::ListEntity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let plan = MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(MixedConstructionConfig::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 3,
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(0, &mut construct);
    extensions.move_providers.bind(1, &mut improve);
    let result = solve_mixed_with_extensions(&p, &n, plan, extensions).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    assert_eq!(result.assignment.basic().values(), &[Some(8)]);
    assert!(result.assignment.lists().lists()[0].is_empty());
}

#[test]
fn dependency_rejections_consume_the_attempt_budget() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    let mut provider = provider();
    provider.entities = vec![0];
    let mut samples = provider.select(&p, &a, 0).unwrap();
    assert_eq!(samples.next(), None);
    assert_eq!(samples.next(), None);
    assert_eq!(a, p.initial_assignment());
}

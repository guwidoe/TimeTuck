//! Original dynamic grouping, range filtering and multi-step provider regressions.
use std::collections::BTreeSet;
use timetuck::*;
fn provider() -> PillarMoveProvider {
    let rank = [0, 2, 1, 3];
    PillarMoveProvider {
        left: PillarSelectionPolicy::sequential(
            2,
            2,
            Some(PillarComparator::new(move |a, b| {
                rank[a[0]].cmp(&rank[b[0]])
            })),
        )
        .unwrap(),
        changes: PillarChangeMode::AssignedOnly,
        swaps: false,
        ..Default::default()
    }
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap()
}
#[test]
fn regrouping_tracks_commits_and_schema_pins_without_value_clones() {
    #[derive(PartialEq, Eq)]
    struct Value(i32);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![Value(0)]),
        Variable::new(vec![Value(0)]),
    ])
    .unwrap();
    let a = Assignment::new(vec![Some(Value(0)), Some(Value(0))]);
    assert_eq!(pillar_groups(&p, &a).unwrap()[0].variables(), [0, 1]);
    let p = basic();
    let mut a = p.initial_assignment();
    let before = pillar_groups(&p, &a).unwrap();
    assert_eq!(before[0].variables(), [0, 1, 2, 3]);
    p.apply(
        &mut a,
        &Move::Change {
            variable: 2,
            value: Some(1),
        },
    )
    .unwrap()
    .commit();
    assert_eq!(
        pillar_groups(&p, &a)
            .unwrap()
            .iter()
            .map(Pillar::variables)
            .collect::<Vec<_>>(),
        [vec![0, 1, 3], vec![2]]
    );
    assert_eq!(before[0].variables(), [0, 1, 2, 3]);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]).with_initial(1),
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]).with_initial(1),
    ])
    .unwrap()
    .with_pillar_schema(PillarSchema::new(vec![vec![0, 1], vec![2, 3], vec![4, 5]]).unwrap())
    .unwrap();
    assert_eq!(
        pillar_groups(&p, &p.initial_assignment()).unwrap()[0].rows(),
        &[vec![0, 1], vec![4, 5]]
    );
    assert!(pillar_groups(&p, &Assignment::new(vec![])).is_err());
    assert!(
        PillarMoveProvider::default()
            .select(&p, &Assignment::new(vec![]))
            .is_err()
    );
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    assert!(
        pillar_groups(&empty, &empty.initial_assignment())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn comparator_windows_drive_two_steps_in_basic_and_stream_search() {
    for stream in [false, true] {
        let p = basic();
        let mut provider = provider();
        let config = LocalSearchConfig {
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("ones", source.port(), |(_, v)| {
                SimpleScore::new(v.unwrap_or(0) as i64)
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
            let mut n = ConstraintNetwork::new(4);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![i], |v| {
                    SimpleScore::new(v[0].copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.completed_steps, 2);
        let best = result.best.unwrap();
        assert_eq!(best.assignment.values(), &[Some(1); 4]);
        assert_eq!(best.score, SimpleScore::new(4));
    }
}
#[test]
fn mixed_backends_regroup_basic_rows_and_keep_lists_unchanged() {
    let p = MixedProblem::new(
        basic(),
        ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![10, 20])]).unwrap(),
    );
    for stream in [false, true] {
        let mut provider = provider();
        let config = MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 2,
                assert_scores: true,
                ..Default::default()
            },
            joint_moves: true,
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            b.score(
                "ones",
                source.basic.port(),
                |(_, v): &(usize, Option<i32>)| SimpleScore::new(v.unwrap_or(0) as i64),
            )
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = MixedConstraintNetwork::new(4, 1, 2);
            for i in 0..4 {
                n.add_constraint(format!("v{i}"), vec![MixedScope::Basic(i)], |v| {
                    let MixedFact::Basic(v) = v[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(v.copied().unwrap_or(0) as i64)
                })
                .unwrap();
            }
            mixed_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.completed_steps, 2);
        let best = result.best.unwrap();
        assert_eq!(best.assignment.basic().values(), &[Some(1); 4]);
        assert_eq!(best.assignment.lists(), p.initial_assignment().lists());
        assert_eq!(best.score, SimpleScore::new(4));
    }
}
#[test]
fn asymmetric_windows_and_null_ranges_match_independent_row_oracle() {
    // Equal-valued rows 0,2,3; row1 differs. Comparator order 2,0,3 creates
    // pairs {0,2} and {0,3}; canonical adjacency would incorrectly include {2,3}.
    let rows = vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6, 7]];
    let initial = vec![
        Some(0),
        None,
        Some(1),
        Some(2),
        Some(0),
        None,
        Some(0),
        None,
    ];
    for restricted in [false, true] {
        for pin in [false, true] {
            let vars = (0..8)
                .map(|i| {
                    let domain = if restricted && i == 4 {
                        vec![0]
                    } else {
                        vec![0, 1, 2]
                    };
                    let v = Variable::new(domain)
                        .allowing_unassigned()
                        .with_pinned(pin && i == 7);
                    match initial[i] {
                        Some(value) => v.with_initial(value),
                        None => v,
                    }
                })
                .collect();
            let p = FiniteProblem::new(vars)
                .unwrap()
                .with_pillar_schema(PillarSchema::new(rows.clone()).unwrap())
                .unwrap();
            let a = Assignment::new(initial.clone());
            p.validate(&a).unwrap();
            let ranks = [1, 3, 0, 2];
            let provider = PillarMoveProvider {
                left: PillarSelectionPolicy::sequential(
                    2,
                    2,
                    Some(PillarComparator::new(move |a, b| {
                        ranks[a[0] / 2].cmp(&ranks[b[0] / 2])
                    })),
                )
                .unwrap(),
                right: PillarSelectionPolicy::whole(),
                ..Default::default()
            };
            let mut expected = BTreeSet::new();
            for members in [vec![0, 2], vec![0, 3]] {
                if pin && members.contains(&3) {
                    continue;
                }
                for col in 0..2 {
                    for target in [Some(0), Some(1), Some(2), None] {
                        if target == initial[col] {
                            continue;
                        }
                        let mut next = initial.clone();
                        for &row in &members {
                            next[2 * row + col] = target;
                        }
                        if p.validate(&Assignment::new(next.clone())).is_ok() {
                            expected.insert(next);
                        }
                    }
                }
                let mut next = initial.clone();
                for &row in &members {
                    next[2 * row..2 * row + 2].copy_from_slice(&initial[2..4]);
                }
                next[2..4].copy_from_slice(&initial[0..2]);
                if p.validate(&Assignment::new(next.clone())).is_ok() {
                    expected.insert(next);
                }
            }
            let actual = provider
                .select(&p, &a)
                .unwrap()
                .map(|m| {
                    let mut next = a.clone();
                    p.apply(&mut next, &m).unwrap().commit();
                    next.values().to_vec()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected, "restricted={restricted}, pin={pin}");
        }
    }
}
#[test]
fn huge_subset_spaces_remain_lazy_and_clone_only_yielded_values() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Eq, PartialEq)]
    struct Value(i32, Rc<Cell<usize>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    let clones = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..80)
            .map(|_| {
                Variable::new(vec![Value(0, clones.clone()), Value(1, clones.clone())])
                    .with_initial(Value(0, clones.clone()))
            })
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let provider = PillarMoveProvider {
        left: PillarSelectionPolicy::subsets(40, 40).unwrap(),
        swaps: false,
        ..Default::default()
    };
    let mut selected = provider.select(&p, &a).unwrap();
    assert_eq!(clones.get(), 0);
    let movement = selected.next().unwrap();
    assert_eq!(clones.get(), 40);
    let Move::Composite(parts) = movement else {
        panic!("expected composite")
    };
    assert_eq!(parts.len(), 40);
}

#[test]
fn impossible_right_size_skips_exponential_left_combinations() {
    let p = FiniteProblem::new(
        (0..81)
            .map(|i| Variable::new(vec![0, 1]).with_initial(i32::from(i == 80)))
            .collect(),
    )
    .unwrap();
    let provider = PillarMoveProvider {
        left: PillarSelectionPolicy::subsets(40, 40).unwrap(),
        right: PillarSelectionPolicy::subsets(2, 2).unwrap(),
        changes: PillarChangeMode::Disabled,
        swaps: true,
    };
    assert!(
        provider
            .select(&p, &p.initial_assignment())
            .unwrap()
            .next()
            .is_none()
    );
}

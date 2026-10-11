// Original planning-value identity and search regressions; no source credit.
use timetuck::*;
const Z: SimpleScore = SimpleScore::ZERO;
fn policy(tenure: usize, fading: bool) -> Acceptance {
    if fading {
        Acceptance::FadingValueTabu {
            hard_tenure: tenure,
            fading_tenure: 0,
            aspiration: false,
            seed: 71,
        }
    } else {
        Acceptance::ValueTabu {
            tenure,
            aspiration: false,
        }
    }
}
#[test]
fn basic_captures_targets_nulls_and_intermediate_composite_writes_without_mutation() {
    let a = Assignment::new(vec![Some(10), Some(20), None]);
    assert_eq!(
        a.planning_values(&Move::Change {
            variable: 0,
            value: Some(30)
        })
        .unwrap(),
        vec![Some(30)]
    );
    assert_eq!(
        a.planning_values(&Move::Swap { left: 0, right: 2 })
            .unwrap(),
        vec![None, Some(10)]
    );
    let m = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(30),
        },
        Move::Swap { left: 0, right: 1 },
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: None,
            },
            Move::Change {
                variable: 1,
                value: Some(30),
            },
        ]),
    ]);
    assert_eq!(
        a.planning_values(&m).unwrap(),
        vec![Some(30), Some(20), None]
    );
    assert_eq!(a.values(), &[Some(10), Some(20), None]);
    let bad = Move::Composite(vec![m, Move::Swap { left: 0, right: 9 }]);
    assert!(a.planning_values(&bad).is_err());
    assert_eq!(a.values(), &[Some(10), Some(20), None]);
    assert!(
        a.planning_values(&Move::Composite(vec![]))
            .unwrap()
            .is_empty()
    );
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![10, 20, 30, 40, 50],
        vec![ListEntity::new(vec![10, 20, 30]), ListEntity::new(vec![40])],
    )
    .unwrap()
    .allowing_unassigned()
}
#[test]
fn list_values_follow_moved_items_not_shifted_neighbors_and_composites_use_current_indices() {
    let p = lists();
    let a = p.initial_assignment();
    let first = ListMove::Change {
        source: pos(0, 0),
        destination: pos(1, 1),
    };
    assert_eq!(p.planning_values(&a, &first).unwrap(), vec![10]);
    let m = ListMove::Composite(vec![
        first,
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 2),
        },
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
    ]);
    assert_eq!(p.planning_values(&a, &m).unwrap(), vec![10, 20, 30, 40]);
    for reverse in [false, true] {
        let m = ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 2,
            destination: pos(1, 1),
            reverse,
        };
        assert_eq!(p.planning_values(&a, &m).unwrap(), vec![10, 20]);
        let m = ListMove::SubListSwap {
            left: ListRange {
                entity: 0,
                start: 0,
                end: 2,
            },
            right: ListRange {
                entity: 1,
                start: 0,
                end: 1,
            },
            reverse,
        };
        assert_eq!(p.planning_values(&a, &m).unwrap(), vec![10, 20, 40]);
    }
    assert_eq!(
        p.planning_values(
            &a,
            &ListMove::Unassign {
                entity: 0,
                start: 1,
                end: 3
            }
        )
        .unwrap(),
        vec![20, 30]
    );
    assert_eq!(
        p.planning_values(
            &a,
            &ListMove::Assign {
                value: 50,
                destination: pos(0, 0)
            }
        )
        .unwrap(),
        vec![50]
    );
    let m = ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 10,
            destination: pos(1, 0),
        },
    ]);
    assert_eq!(p.planning_values(&a, &m).unwrap(), vec![10]);
    // Final-state range validation must allow transient cross-range values in
    // an ordered composite that restores both lists.
    let ranged = ListProblem::new(
        vec![10, 40],
        vec![
            ListEntity::new(vec![10]).with_value_range(vec![10]),
            ListEntity::new(vec![40]).with_value_range(vec![40]),
        ],
    )
    .unwrap();
    let swap = ListMove::Swap {
        left: pos(0, 0),
        right: pos(1, 0),
    };
    assert!(
        ranged
            .planning_values(&ranged.initial_assignment(), &swap)
            .is_err()
    );
    assert_eq!(
        ranged
            .planning_values(
                &ranged.initial_assignment(),
                &ListMove::Composite(vec![swap.clone(), swap])
            )
            .unwrap(),
        vec![10, 40]
    );
    assert_eq!(a, p.initial_assignment());
    let bad = ListMove::Composite(vec![
        m,
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(9, 0),
        },
    ]);
    assert!(p.planning_values(&a, &bad).is_err());
    assert_eq!(a, p.initial_assignment());
}
#[test]
fn mixed_value_namespaces_remain_independent_and_preserve_domain_validation() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![10, 20]).with_initial(10)]).unwrap();
    let p = MixedProblem::new(basic, lists());
    let a = p.initial_assignment();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(20),
        },
        lists: ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 1),
        },
    };
    let values = p.planning_values(&a, &m).unwrap();
    assert_eq!(
        values,
        vec![
            MixedPlanningValue::Basic(Some(20)),
            MixedPlanningValue::List(10),
            MixedPlanningValue::List(20)
        ]
    );
    let mut accept = policy(2, false).start_with_values(Z).unwrap();
    accept.step_ended(
        Z,
        &[],
        &0,
        &[MixedPlanningValue::<i32, i32>::Basic(Some(20))],
    );
    assert!(accept.accepts(Z, Z, &[], &0, &[MixedPlanningValue::List(20)]));
    assert!(!accept.accepts(Z, Z, &[], &0, &values));
    let bad = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(99),
        },
        ..m
    };
    assert!(p.planning_values(&a, &bad).is_err());
    assert_eq!(a, p.initial_assignment());
}
#[test]
fn independent_chronology_tracks_multiple_values_null_refresh_and_strict_aspiration() {
    for tenure in [1, 2, 7, usize::MAX] {
        for fading in [false, true] {
            let mut a = policy(tenure, fading).start_with_values(Z).unwrap();
            let mut history: Vec<Vec<Option<i32>>> = Vec::new();
            for step in 0..120usize {
                for key in [None, Some(0), Some(1), Some(2)] {
                    let expected = !history
                        .iter()
                        .enumerate()
                        .any(|(i, v)| step - i <= tenure && v.contains(&key));
                    assert_eq!(
                        a.accepts(Z, SimpleScore::new(9), &[99], &0, &[key]),
                        expected
                    );
                }
                let keys = match step % 4 {
                    0 => vec![None, Some(0), None],
                    1 => vec![Some(1), Some(2)],
                    2 => vec![],
                    _ => vec![Some(0)],
                };
                a.step_ended(Z, &[], &0, &keys);
                history.push(keys);
            }
        }
    }
    let mut a = Acceptance::ValueTabu {
        tenure: 2,
        aspiration: true,
    }
    .start_with_values(SimpleScore::new(10))
    .unwrap();
    a.step_ended(SimpleScore::new(5), &[], &0, &[None::<i32>]);
    assert!(!a.accepts(Z, SimpleScore::new(10), &[], &0, &[None]));
    assert!(a.accepts(Z, SimpleScore::new(11), &[], &0, &[None]));
    a.step_ended(SimpleScore::new(11), &[], &0, &[None]);
    assert!(!a.accepts(Z, SimpleScore::new(11), &[], &0, &[None]));
}
#[test]
fn context_and_configuration_errors_precede_state_allocation_and_scoring() {
    assert!(matches!(
        policy(1, false).start(Z),
        Err(AcceptanceError::ValueContextRequired)
    ));
    assert!(matches!(
        policy(1, false).start_with_moves::<i32>(Z),
        Err(AcceptanceError::ValueContextRequired)
    ));
    let bad = Acceptance::FadingValueTabu {
        hard_tenure: usize::MAX,
        fading_tenure: 1,
        aspiration: false,
        seed: 0,
    };
    assert!(matches!(
        ValueAcceptor::<_, i32, i32>::from_policies(
            &[
                Acceptance::LateAcceptance {
                    history_size: usize::MAX,
                    hill_climbing: false
                },
                bad
            ],
            Z
        ),
        Err(AcceptanceError::TabuTenureOverflow)
    ));
    let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("never", vec![0], |_| -> SimpleScore {
        panic!("invalid configuration scored")
    })
    .unwrap();
    assert!(matches!(
        local_search(
            &p,
            &n,
            LocalSearchConfig {
                acceptance: policy(0, false),
                max_evaluations: 0,
                ..Default::default()
            }
        ),
        Err(LocalSearchError::Acceptance(AcceptanceError::EmptyTabu))
    ));
}

fn basic_values(a: &Assignment<i32>, m: &Move<i32>) -> Vec<Option<i32>> {
    match m {
        Move::Change { value, .. } => vec![*value],
        Move::Swap { left, right } => vec![a.values()[*left], a.values()[*right]],
        Move::Composite(v) if v.is_empty() => vec![],
        _ => panic!("reference supports primitive neighborhoods"),
    }
}
fn list_values(a: &ListAssignment<i32>, m: &ListMove<i32>) -> Vec<i32> {
    match m {
        ListMove::Change { source, .. } => vec![a.lists()[source.entity][source.index]],
        ListMove::Swap { left, right } => vec![
            a.lists()[left.entity][left.index],
            a.lists()[right.entity][right.index],
        ],
        ListMove::Composite(v) if v.is_empty() => vec![],
        _ => panic!("reference supports primitive complete-list neighborhoods"),
    }
}
fn reference<A, M, K: Eq>(
    mut a: A,
    moves: impl Fn(&A) -> Vec<M>,
    keys: impl Fn(&A, &M) -> Vec<K>,
    apply: impl Fn(&mut A, &M),
    tenure: usize,
) -> u64 {
    let mut history: Vec<Vec<K>> = Vec::new();
    for step in 0..12 {
        let selected = moves(&a).into_iter().find_map(|m| {
            let k = keys(&a, &m);
            if history
                .iter()
                .rev()
                .take(tenure)
                .flatten()
                .any(|past| k.contains(past))
            {
                None
            } else {
                Some((m, k))
            }
        });
        let Some((m, k)) = selected else { return step };
        apply(&mut a, &m);
        history.push(k);
    }
    12
}
#[test]
fn value_keys_control_all_six_backends_against_independent_selected_value_histories() {
    let basic = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(0),
    ])
    .unwrap();
    let lists = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    for tenure in [1, 2, 10] {
        let bs = reference(
            basic.initial_assignment(),
            |a| {
                change_moves(&basic, a)
                    .unwrap()
                    .chain(swap_moves(&basic, a).unwrap())
                    .collect()
            },
            basic_values,
            |a, m| basic.apply(a, m).unwrap().commit(),
            tenure,
        );
        let ls = reference(
            lists.initial_assignment(),
            |a| {
                list_moves(&lists, a, Neighborhood::ChangeAndSwap)
                    .unwrap()
                    .collect()
            },
            list_values,
            |a, m| lists.apply(a, m).unwrap().commit(),
            tenure,
        );
        let ms = reference(
            mixed.initial_assignment(),
            |a| {
                mixed_moves(&mixed, a, Neighborhood::ChangeAndSwap, true)
                    .unwrap()
                    .collect()
            },
            |a, m| {
                basic_values(a.basic(), &m.basic)
                    .into_iter()
                    .map(|v| (false, v))
                    .chain(
                        list_values(a.lists(), &m.lists)
                            .into_iter()
                            .map(|v| (true, Some(v))),
                    )
                    .collect()
            },
            |a, m| mixed.apply(a, m).unwrap().commit(),
            tenure,
        );
        if tenure == 10 {
            assert_eq!(bs, 3);
            assert_eq!(ls, 2);
        }
        for fading in [false, true] {
            let p = policy(tenure, fading);
            let extra = [p];
            for additional in [false, true] {
                let c = LocalSearchConfig {
                    acceptance: if additional {
                        Acceptance::HillClimbing
                    } else {
                        p
                    },
                    additional_acceptance: if additional { &extra } else { &[] },
                    max_steps: 12,
                    max_evaluations: 10000,
                    assert_scores: true,
                    ..Default::default()
                };
                let n = ConstraintNetwork::<i32, SimpleScore>::new(2);
                let a = local_search(&basic, &n, c.clone()).unwrap();
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                let b = stream_local_search(&basic, b.build(), s, c.clone()).unwrap();
                assert_eq!(a.completed_steps, bs);
                assert_eq!(b.completed_steps, bs);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
                let n = ListConstraintNetwork::<i32, SimpleScore>::new(2, 2);
                let a = list_local_search_from(&lists, &n, lists.initial_assignment(), c.clone())
                    .unwrap();
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = ListStreamSources::new(&mut b);
                let b = list_stream_local_search_from(
                    &lists,
                    b.build(),
                    s,
                    lists.initial_assignment(),
                    c.clone(),
                )
                .unwrap();
                assert_eq!(a.completed_steps, ls);
                assert_eq!(b.completed_steps, ls);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
                let n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(2, 2, 2);
                let c = MixedLocalSearchConfig {
                    search: c,
                    joint_moves: true,
                };
                let a = mixed_local_search_from(&mixed, &n, mixed.initial_assignment(), c.clone())
                    .unwrap();
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = MixedStreamSources::new(&mut b);
                let b = mixed_stream_local_search_from(
                    &mixed,
                    b.build(),
                    s,
                    mixed.initial_assignment(),
                    c,
                )
                .unwrap();
                assert_eq!(a.completed_steps, ms);
                assert_eq!(b.completed_steps, ms);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
            }
        }
    }
}
#[test]
fn skipped_fading_draws_and_completed_fallback_updates_keep_child_memories_separate() {
    let policies = [
        Acceptance::StrictImprovement,
        Acceptance::FadingValueTabu {
            hard_tenure: 2,
            fading_tenure: 4,
            aspiration: false,
            seed: 27,
        },
    ];
    let mut a = ValueAcceptor::from_policies(&policies, SimpleScore::new(10)).unwrap();
    let mut b = a.clone();
    for state in [&mut a, &mut b] {
        for v in [1, 0, 0] {
            state.step_ended(Z, &[], &0, &[v]);
        }
    }
    for _ in 0..100 {
        assert!(!a.accepts(Z, SimpleScore::new(-1), &[], &0, &[1]));
    }
    let outcomes: Vec<_> = (0..100)
        .map(|_| {
            let x = a.accepts(Z, SimpleScore::new(1), &[], &0, &[1]);
            assert_eq!(x, b.accepts(Z, SimpleScore::new(1), &[], &0, &[1]));
            x
        })
        .collect();
    assert!(outcomes.contains(&true) && outcomes.contains(&false));
    a.step_ended(SimpleScore::new(-1), &[], &0, &[2]);
    assert!(!a.accepts(SimpleScore::new(-1), Z, &[], &0, &[2]));
    assert!(a.accepts(SimpleScore::new(-1), Z, &[], &0, &[9]));
    let policies = [
        policy(2, false),
        Acceptance::MoveTabu {
            tenure: 2,
            aspiration: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: false,
        },
    ];
    let mut a = ValueAcceptor::from_policies(&policies, Z).unwrap();
    a.step_ended(Z, &[0], &1, &[2]);
    assert!(!a.accepts(Z, Z, &[9], &9, &[2]));
    assert!(!a.accepts(Z, Z, &[9], &1, &[9]));
    assert!(!a.accepts(Z, Z, &[0], &9, &[9]));
    assert!(a.accepts(Z, Z, &[9], &9, &[9]));
}

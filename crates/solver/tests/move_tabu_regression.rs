// Original native identity, state and all-backend regressions; no source credit.
use timetuck::*;
const ZERO: SimpleScore = SimpleScore::ZERO;
fn policy(tenure: usize, fading: bool, aspiration: bool) -> Acceptance {
    if fading {
        Acceptance::FadingMoveTabu {
            hard_tenure: tenure,
            fading_tenure: 4,
            aspiration,
            seed: 47,
        }
    } else {
        Acceptance::MoveTabu { tenure, aspiration }
    }
}
fn identities<M: Eq + Clone>(moves: &[M]) {
    for fading in [false, true] {
        let mut a = policy(10, fading, false).start_with_moves(ZERO).unwrap();
        a.step_ended(ZERO, &[0], &moves[0]);
        for (i, m) in moves.iter().enumerate() {
            assert_eq!(a.accepts(ZERO, ZERO, &[0], m), i != 0);
        }
        let mut fresh = policy(10, fading, false).start_with_moves(ZERO).unwrap();
        assert!(fresh.accepts(ZERO, ZERO, &[0], &moves[0]));
    }
}
#[test]
fn structural_descriptors_distinguish_values_orientation_positions_and_components() {
    let a = Move::Change {
        variable: 0,
        value: Some(String::from("a")),
    };
    identities(&[
        a.clone(),
        Move::Change {
            variable: 0,
            value: Some(String::from("b")),
        },
        Move::Change {
            variable: 1,
            value: Some(String::from("a")),
        },
        Move::Change {
            variable: 0,
            value: None,
        },
        Move::Composite(vec![a.clone()]),
    ]);
    identities::<Move<i32>>(&[
        Move::Swap { left: 0, right: 1 },
        Move::Swap { left: 1, right: 0 },
    ]);
    let pos = |entity, index| ListPosition { entity, index };
    let l = ListMove::<i32>::Change {
        source: pos(0, 0),
        destination: pos(1, 0),
    };
    identities(&[
        l.clone(),
        ListMove::Change {
            source: pos(1, 0),
            destination: pos(0, 0),
        },
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 0),
        },
        ListMove::Composite(vec![l.clone()]),
    ]);
    identities(&[
        MixedMove {
            basic: a.clone(),
            lists: l.clone(),
        },
        MixedMove {
            basic: a.clone(),
            lists: ListMove::Composite(vec![]),
        },
        MixedMove {
            basic: Move::Composite(vec![]),
            lists: l.clone(),
        },
    ]);
    identities(&[
        Move::Composite(vec![a.clone(), Move::Swap { left: 0, right: 1 }]),
        Move::Composite(vec![Move::Swap { left: 0, right: 1 }, a]),
    ]);
}
#[test]
fn chronological_reference_checks_expiry_refresh_trials_and_global_best() {
    for tenure in [1, 2, 7, usize::MAX] {
        for aspiration in [false, true] {
            let mut a = policy(tenure, false, aspiration)
                .start_with_moves(SimpleScore::new(10))
                .unwrap();
            let mut selected = Vec::new();
            let mut best = 10;
            for step in 0..180usize {
                for key in 0..9 {
                    for score in [-8, 0, 10, 11, 30] {
                        let blocked = selected
                            .iter()
                            .enumerate()
                            .any(|(i, &m)| m == key && step - i <= tenure);
                        let expected = (aspiration && score > best) || !blocked;
                        assert_eq!(
                            a.accepts(
                                SimpleScore::new(-100),
                                SimpleScore::new(score),
                                &[key],
                                &key
                            ),
                            expected
                        );
                    }
                }
                let key = (step * step + step / 7) % 9;
                let score = step as i64 % 23 - 4;
                a.step_ended(SimpleScore::new(score), &[], &key);
                selected.push(key);
                best = best.max(score);
            }
        }
    }
}
#[test]
fn skipped_fading_trials_preserve_draws_and_rejected_steps_still_record_moves() {
    let policies = [Acceptance::StrictImprovement, policy(2, true, false)];
    let mut a = MoveAcceptor::from_policies(&policies, SimpleScore::new(10)).unwrap();
    let mut b = a.clone();
    for state in [&mut a, &mut b] {
        for m in [1, 0, 0] {
            state.step_ended(ZERO, &[], &m);
        }
    }
    for _ in 0..100 {
        assert!(!a.accepts(ZERO, SimpleScore::new(-1), &[], &1));
    }
    let draws: Vec<_> = (0..100)
        .map(|_| {
            let x = a.accepts(ZERO, SimpleScore::new(1), &[], &1);
            assert_eq!(x, b.accepts(ZERO, SimpleScore::new(1), &[], &1));
            x
        })
        .collect();
    assert!(draws.contains(&true) && draws.contains(&false));
    // Even a step rejected by the first child updates the later tabu state.
    a.step_ended(SimpleScore::new(-1), &[], &2);
    assert!(!a.accepts(SimpleScore::new(-1), ZERO, &[], &2));
    assert!(a.accepts(SimpleScore::new(-1), ZERO, &[], &9));
}
#[test]
fn only_selected_move_descriptors_are_cloned() {
    use std::{cell::Cell, rc::Rc};
    struct Key(Rc<Cell<usize>>, usize);
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone(), self.1)
        }
    }
    impl PartialEq for Key {
        fn eq(&self, o: &Self) -> bool {
            self.1 == o.1
        }
    }
    impl Eq for Key {}
    for p in [
        Acceptance::HillClimbing,
        policy(2, false, false),
        policy(2, true, false),
    ] {
        let calls = Rc::new(Cell::new(0));
        let key = Key(calls.clone(), 7);
        let mut a = p.start_with_moves(ZERO).unwrap();
        for _ in 0..100 {
            assert!(a.accepts(ZERO, ZERO, &[], &key));
        }
        assert_eq!(calls.get(), 0);
        a.step_ended(ZERO, &[], &key);
        assert_eq!(calls.get(), usize::from(p != Acceptance::HillClimbing));
    }
}
fn reference<A, M: Eq + Clone>(
    mut a: A,
    moves: impl Fn(&A) -> Vec<M>,
    apply: impl Fn(&mut A, &M),
    tenure: usize,
) -> u64 {
    let mut selected = Vec::new();
    for step in 0..12 {
        let found = moves(&a)
            .into_iter()
            .find(|m| !selected.iter().rev().take(tenure).any(|past| past == m));
        let Some(m) = found else { return step };
        apply(&mut a, &m);
        selected.push(m);
    }
    12
}
#[test]
fn complete_move_identity_controls_all_six_search_paths_and_extra_policies() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let lists = ListProblem::new(
        vec![7, 8],
        vec![ListEntity::new(vec![7, 8]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    for tenure in [1, 2, 10] {
        let basic_steps = reference(
            basic.initial_assignment(),
            |a| change_moves(&basic, a).unwrap().collect(),
            |a, m| basic.apply(a, m).unwrap().commit(),
            tenure,
        );
        let list_steps = reference(
            lists.initial_assignment(),
            |a| {
                list_moves(&lists, a, Neighborhood::ChangeAndSwap)
                    .unwrap()
                    .collect()
            },
            |a, m| lists.apply(a, m).unwrap().commit(),
            tenure,
        );
        let mixed_steps = reference(
            mixed.initial_assignment(),
            |a| {
                mixed_moves(&mixed, a, Neighborhood::ChangeAndSwap, true)
                    .unwrap()
                    .collect()
            },
            |a, m| mixed.apply(a, m).unwrap().commit(),
            tenure,
        );
        if tenure == 10 {
            assert_eq!(basic_steps, 3);
        }
        for fading in [false, true] {
            // No fading interval: exact hard-tabu equivalence, including expiry.
            let p = if fading {
                Acceptance::FadingMoveTabu {
                    hard_tenure: tenure,
                    fading_tenure: 0,
                    aspiration: false,
                    seed: 9,
                }
            } else {
                policy(tenure, false, false)
            };
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
                let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
                let a = local_search(&basic, &n, c).unwrap();
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                let b = stream_local_search(&basic, b.build(), source, c).unwrap();
                assert_eq!(a.completed_steps, basic_steps);
                assert_eq!(b.completed_steps, basic_steps);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
                let n = ListConstraintNetwork::<i32, SimpleScore>::new(2, 2);
                let a = list_local_search_from(&lists, &n, lists.initial_assignment(), c).unwrap();
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = ListStreamSources::new(&mut b);
                let b = list_stream_local_search_from(
                    &lists,
                    b.build(),
                    s,
                    lists.initial_assignment(),
                    c,
                )
                .unwrap();
                assert_eq!(a.completed_steps, list_steps);
                assert_eq!(b.completed_steps, list_steps);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
                let n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(1, 2, 2);
                let c = MixedLocalSearchConfig {
                    search: c,
                    joint_moves: true,
                };
                let a = mixed_local_search_from(&mixed, &n, mixed.initial_assignment(), c).unwrap();
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
                assert_eq!(a.completed_steps, mixed_steps);
                assert_eq!(b.completed_steps, mixed_steps);
                assert_eq!(a.best, b.best);
                assert_eq!(a.termination, b.termination);
            }
        }
    }
}
#[test]
fn invalid_move_policies_fail_before_work_and_require_typed_standalone_context() {
    for p in [policy(1, false, true), policy(1, true, true)] {
        assert!(matches!(
            p.start(ZERO),
            Err(AcceptanceError::MoveContextRequired)
        ));
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("unused", vec![0], |_| -> SimpleScore {
        panic!("scored invalid policy")
    })
    .unwrap();
    for (invalid, error) in [
        (policy(0, false, true), AcceptanceError::EmptyTabu),
        (
            Acceptance::FadingMoveTabu {
                hard_tenure: usize::MAX,
                fading_tenure: 1,
                aspiration: true,
                seed: 0,
            },
            AcceptanceError::TabuTenureOverflow,
        ),
    ] {
        let policies = [
            Acceptance::LateAcceptance {
                history_size: usize::MAX,
                hill_climbing: false,
            },
            invalid,
        ];
        assert!(matches!(MoveAcceptor::<_,i32>::from_policies(&policies,ZERO),Err(e) if e==error));
        assert!(
            matches!(local_search(&p,&n,LocalSearchConfig {acceptance:invalid,max_evaluations:0,..Default::default()}),Err(LocalSearchError::Acceptance(e)) if e==error)
        );
    }
}

#[test]
fn typed_conjunction_preserves_identity_and_variable_move_memory_independently() {
    let mut empty = MoveAcceptor::<_, i32>::from_policies(&[], ZERO).unwrap();
    assert!(empty.accepts(ZERO, SimpleScore::new(-5), &[], &1));
    let mut many = MoveAcceptor::from_policies(&vec![Acceptance::HillClimbing; 257], ZERO).unwrap();
    assert!(many.accepts(ZERO, ZERO, &[], &1));
    assert!(!many.accepts(ZERO, SimpleScore::new(-1), &[], &1));
    let policies = [
        policy(2, false, false),
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: false,
        },
    ];
    let mut a = MoveAcceptor::from_policies(&policies, ZERO).unwrap();
    a.step_ended(ZERO, &[0], &7);
    assert!(!a.accepts(ZERO, ZERO, &[1], &7)); // Same descriptor, different variable.
    assert!(!a.accepts(ZERO, ZERO, &[0], &8)); // Different descriptor, same variable.
    assert!(a.accepts(ZERO, ZERO, &[1], &8));
}

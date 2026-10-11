//! Original native integration cases; no upstream method credit.
use ProviderConstructionTermination as Stop;
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};
use timetuck::*;

type Config<'a> = ProviderConstructionConfig<'a>;
type BasicResult = ProviderConstructionResult<Assignment<i32>, SimpleScore>;
type ListResult = ProviderConstructionResult<ListAssignment<i32>, SimpleScore>;
type MixedResult = ProviderConstructionResult<MixedAssignment<i32, i32>, SimpleScore>;
fn change(variable: usize, value: i32) -> Move<i32> {
    Move::Change {
        variable,
        value: Some(value),
    }
}
fn insert(value: i32, entity: usize, index: usize) -> ListMove<i32> {
    ListMove::Assign {
        value,
        destination: ListPosition { entity, index },
    }
}
fn basic(
    p: &FiniteProblem<i32>,
    initial: Assignment<i32>,
    stream: bool,
    c: Config,
    provider: &mut dyn MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>>,
) -> BasicResult {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        b.score("value", s.port(), |r| {
            SimpleScore::new(-i64::from(r.1.unwrap_or(0)))
        })
        .unwrap();
        stream_construction_from_with_provider(p, b.build(), s, initial, c, provider).unwrap()
    } else {
        let mut n = ConstraintNetwork::new(p.variables().len());
        for i in 0..p.variables().len() {
            n.add_constraint(format!("value{i}"), vec![i], |r| {
                SimpleScore::new(-i64::from(r[0].copied().unwrap_or(0)))
            })
            .unwrap();
        }
        construction_from_with_provider(p, &n, initial, c, provider).unwrap()
    }
}
fn lists(
    p: &ListProblem<i32>,
    initial: ListAssignment<i32>,
    stream: bool,
    c: Config,
    provider: &mut dyn MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>>,
) -> ListResult {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("positions", s.entities.port(), |(_, list)| {
            SimpleScore::new(
                list.iter()
                    .enumerate()
                    .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                    .sum(),
            )
        })
        .unwrap();
        list_stream_construction_from_with_provider(p, b.build(), s, initial, c, provider).unwrap()
    } else {
        let mut n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
        for i in 0..p.entities().len() {
            n.add_constraint(format!("positions{i}"), vec![ListScope::Entity(i)], |r| {
                let ListFact::Entity(list) = &r[0] else {
                    unreachable!()
                };
                SimpleScore::new(
                    list.iter()
                        .enumerate()
                        .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                        .sum(),
                )
            })
            .unwrap();
        }
        list_construction_from_with_provider(p, &n, initial, c, provider).unwrap()
    }
}
fn mixed(
    p: &MixedProblem<i32, i32>,
    stream: bool,
    c: Config,
    provider: &mut dyn MoveProvider<MixedProblem<i32, i32>, MixedAssignment<i32, i32>, MixedMove<i32, i32>>,
) -> MixedResult {
    if stream {
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let joined = b
            .join_equal(s.basic.port(), s.lists.entities.port(), |r| r.0, |r| r.0)
            .unwrap();
        b.score("joint", &joined, |((_, value), (_, list))| {
            SimpleScore::new(-i64::from(value.unwrap_or(0)) * list.len() as i64)
        })
        .unwrap();
        mixed_stream_construction_from_with_provider(
            p,
            b.build(),
            s,
            p.initial_assignment(),
            c,
            provider,
        )
        .unwrap()
    } else {
        let mut n = MixedConstraintNetwork::new(1, 1, p.lists().values().len());
        n.add_constraint(
            "joint",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |r| {
                let (MixedFact::Basic(value), MixedFact::ListEntity(list)) = (&r[0], &r[1]) else {
                    unreachable!()
                };
                SimpleScore::new(-i64::from(value.copied().unwrap_or(0)) * list.len() as i64)
            },
        )
        .unwrap();
        mixed_construction_from_with_provider(p, &n, p.initial_assignment(), c, provider).unwrap()
    }
}
#[test]
fn assignment_sources_drive_real_construction_and_leave_a_local_search_start() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![7, 8]).with_initial(7).with_pinned(true),
        Variable::new(vec![2, 1]),
        Variable::new(vec![4, 3]),
        Variable::new(vec![5]).allowing_unassigned(),
    ])
    .unwrap();
    for stream in [false, true] {
        let seen = RefCell::new(vec![]);
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, step| {
            seen.borrow_mut().push((step, a.clone()));
            let context = BasicSelectionContext::new(p, a).unwrap();
            let variable = BasicEntitySource(BasicEntitySelection::Uninitialized)
                .open(&context)
                .next()
                .unwrap();
            let mut values =
                GatedSelectionSource::new(BasicValueSource, |c: &BasicValueContext<'_, i32>| {
                    c.needs_initialization()
                });
            values
                .open(&context.variable(variable).unwrap())
                .map(|value| Move::Change { variable, value })
                .collect()
        };
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                assert_scores: true,
                ..Config::default()
            },
            &mut provider,
        );
        assert_eq!(r.assignment.values(), &[Some(7), Some(1), Some(3), None]);
        assert_eq!(r.score, Some(SimpleScore::new(-11)));
        assert_eq!(
            (
                r.evaluations,
                r.scanned_moves,
                r.completed_steps,
                r.rejected_moves
            ),
            (5, 4, 2, 0)
        );
        assert_eq!(r.termination, Stop::Completed);
        assert!(r.complete);
        assert_eq!(seen.borrow()[1].0, 1);
        assert_eq!(seen.borrow()[1].1.values()[1], Some(1));
        let n = ConstraintNetwork::<_, SimpleScore>::new(4);
        assert!(
            local_search_from(
                &p,
                &n,
                r.assignment,
                LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }
            )
            .is_ok()
        );
    }
}
#[test]
fn composite_candidates_initialize_multiple_variables_without_replaying_winner() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    for stream in [false, true] {
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            vec![
                change(0, 1),
                Move::Composite(vec![change(0, 2), change(1, 2)]),
            ]
        };
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config::default(),
            &mut provider,
        );
        assert_eq!(r.assignment.values(), &[Some(2), Some(2)]);
        assert_eq!(r.score, Some(SimpleScore::new(-4)));
        assert_eq!((r.completed_steps, r.evaluations), (1, 3));
    }
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("calls", vec![0, 1], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::new(0)
    })
    .unwrap();
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        vec![
            Move::Composite(vec![change(0, 1), change(1, 1)]),
            Move::Composite(vec![change(0, 2), change(1, 2)]),
        ]
    };
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        Config::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1), Some(1)]); // Stable tie.
    assert_eq!(calls.get(), 3);
    assert_eq!(r.constraint_evaluations, 3);
}
#[test]
fn existing_values_pins_invalid_domains_and_noops_are_filtered_before_scoring() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![3, 4]),
    ])
    .unwrap();
    for stream in [false, true] {
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            vec![
                change(5, 1),
                change(1, 999),
                change(0, 2),
                Move::Composite(vec![]),
                Move::Composite(vec![change(0, 2), change(1, 3)]),
                change(1, 4),
                change(1, 3),
            ]
        };
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        );
        assert_eq!(r.assignment.values(), &[Some(1), Some(3)]);
        assert_eq!(
            (r.scanned_moves, r.rejected_moves, r.evaluations),
            (7, 5, 3)
        );
    }
}
#[test]
fn list_construction_preserves_owners_relative_order_prefixes_and_ranges() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![
            ListEntity::new(vec![1, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3, 4]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap();
    for stream in [false, true] {
        let mut provider = |p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            let context = ListSelectionContext::new(p, a).unwrap();
            let value = ListValueSource(ListValueSelection::Unassigned)
                .open(&context)
                .next()
                .unwrap();
            let mut moves = vec![insert(value, 0, 0), insert(value, 1, 0), insert(99, 0, 1)];
            // Legal final model state, but reorders two pre-existing values.
            moves.push(ListMove::Composite(vec![
                ListMove::Swap {
                    left: ListPosition {
                        entity: 0,
                        index: 1,
                    },
                    right: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                },
                insert(value, 0, 2),
            ]));
            moves.extend((1..=a.lists()[0].len()).map(|i| insert(value, 0, i)));
            moves
        };
        let r = lists(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        );
        assert_eq!(r.assignment.lists(), &[vec![1, 2, 3, 4], vec![5]]);
        assert!(r.complete);
        assert_eq!(r.termination, Stop::Completed);
        assert_eq!(r.rejected_moves, 8);
        assert_eq!(r.evaluations, 6);
    }
}
#[test]
fn valid_model_relocation_or_reordering_is_not_construction_progress() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    for stream in [false, true] {
        let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| {
            vec![
                ListMove::Composite(vec![
                    ListMove::Swap {
                        left: ListPosition {
                            entity: 0,
                            index: 0,
                        },
                        right: ListPosition {
                            entity: 0,
                            index: 1,
                        },
                    },
                    insert(4, 0, 2),
                ]),
                ListMove::Composite(vec![
                    ListMove::Change {
                        source: ListPosition {
                            entity: 0,
                            index: 1,
                        },
                        destination: ListPosition {
                            entity: 1,
                            index: 1,
                        },
                    },
                    insert(4, 0, 1),
                ]),
                insert(4, 0, 1),
            ]
        };
        let r = lists(
            &p,
            p.initial_assignment(),
            stream,
            Config::default(),
            &mut provider,
        );
        assert_eq!(r.assignment.lists(), &[vec![1, 4, 2], vec![3]]);
        assert_eq!((r.rejected_moves, r.evaluations), (2, 2));
    }
}
#[test]
fn mixed_joint_moves_compete_as_whole_candidates_in_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap(),
        ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![])]).unwrap(),
    );
    for stream in [false, true] {
        let mut provider = |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
            vec![
                MixedMove {
                    basic: change(0, 1),
                    lists: ListMove::Composite(vec![]),
                },
                MixedMove {
                    basic: change(0, 2),
                    lists: ListMove::Composite(vec![insert(10, 0, 0), insert(20, 0, 1)]),
                },
                MixedMove {
                    basic: change(0, 1),
                    lists: ListMove::Composite(vec![insert(20, 0, 0), insert(10, 0, 1)]),
                },
            ]
        };
        let r = mixed(
            &p,
            stream,
            Config {
                assert_scores: true,
                ..Default::default()
            },
            &mut provider,
        );
        assert!(r.complete);
        assert_eq!(r.score, Some(SimpleScore::new(-2)));
        assert_eq!(r.assignment.basic().values(), &[Some(1)]);
        assert_eq!(r.assignment.lists().lists(), &[vec![20, 10]]);
        assert_eq!((r.completed_steps, r.evaluations), (1, 4));
    }
}
#[test]
fn optional_initialization_is_opt_in_and_score_nonworsening() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![-1, 1]).allowing_unassigned(),
        Variable::new(vec![2]).allowing_unassigned(),
    ])
    .unwrap();
    for stream in [false, true] {
        for o in [
            OptionalConstruction::Preserve,
            OptionalConstruction::NonWorsening,
        ] {
            let calls = Cell::new(0);
            let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
                calls.set(calls.get() + 1);
                vec![change(0, 1), change(0, -1), change(1, 2)]
            };
            let r = basic(
                &p,
                p.initial_assignment(),
                stream,
                Config {
                    optional: o,
                    ..Default::default()
                },
                &mut provider,
            );
            assert!(r.complete);
            if o == OptionalConstruction::Preserve {
                assert_eq!(calls.get(), 0);
                assert_eq!(r.termination, Stop::Completed);
            } else {
                assert_eq!(r.assignment.values(), &[Some(-1), None]);
                assert_eq!(r.score, Some(SimpleScore::new(1)));
                assert_eq!(r.termination, Stop::NoCandidates);
            }
        }
    }
    let p = ListProblem::new(vec![-1, 2], vec![ListEntity::new(vec![])])
        .unwrap()
        .allowing_unassigned();
    for stream in [false, true] {
        let mut provider = |_: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64| {
            vec![
                insert(-1, 0, a.lists()[0].len()),
                insert(2, 0, a.lists()[0].len()),
            ]
        };
        let r = lists(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                optional: OptionalConstruction::NonWorsening,
                ..Default::default()
            },
            &mut provider,
        );
        assert_eq!(r.assignment.lists(), &[vec![2]]);
        assert!(r.complete);
        assert_eq!(r.termination, Stop::NoCandidates);
    }
}
#[test]
fn step_and_score_limits_keep_evaluated_prefix_winners_and_allow_resume() {
    let p = FiniteProblem::new(vec![Variable::new(vec![3, 2, 1]); 2]).unwrap();
    for stream in [false, true] {
        for (config, reason, value, evaluations, scanned) in [
            (
                Config {
                    max_steps: Some(1),
                    ..Default::default()
                },
                Stop::StepLimit,
                1,
                4,
                3,
            ),
            (
                Config {
                    max_evaluations: 3,
                    ..Default::default()
                },
                Stop::EvaluationLimit,
                2,
                3,
                2,
            ),
            (
                Config {
                    max_move_evaluations: Some(1),
                    ..Default::default()
                },
                Stop::MoveEvaluationLimit,
                3,
                2,
                1,
            ),
            (
                Config {
                    max_scanned_moves: 2,
                    ..Default::default()
                },
                Stop::ScanLimit,
                2,
                3,
                2,
            ),
        ] {
            let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                let i = a.values().iter().position(Option::is_none).unwrap();
                vec![change(i, 3), change(i, 2), change(i, 1)]
            };
            let r = basic(&p, p.initial_assignment(), stream, config, &mut provider);
            assert_eq!(r.termination, reason);
            assert_eq!(r.assignment.values(), &[Some(value), None]);
            assert_eq!((r.evaluations, r.scanned_moves), (evaluations, scanned));
            let resumed = basic(&p, r.assignment, stream, Config::default(), &mut provider);
            assert!(resumed.complete);
            assert_eq!(resumed.assignment.values(), &[Some(value), Some(1)]);
        }
    }
}
struct EndlessInvalid(Cell<u64>);
impl<P, A> MoveProvider<P, A, Move<i32>> for EndlessInvalid {
    fn moves<'a>(
        &'a mut self,
        _: &'a P,
        _: &'a A,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(std::iter::from_fn(move || {
            self.0.set(self.0.get() + 1);
            Some(change(999, 0))
        }))
    }
}
#[test]
fn raw_scan_limit_stops_infinite_rejected_moves_without_score_work() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    for stream in [false, true] {
        let mut provider = EndlessInvalid(Cell::new(0));
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                max_scanned_moves: 17,
                ..Default::default()
            },
            &mut provider,
        );
        assert_eq!(r.termination, Stop::ScanLimit);
        assert_eq!(
            (
                r.scanned_moves,
                r.rejected_moves,
                r.evaluations,
                r.completed_steps
            ),
            (17, 17, 1, 0)
        );
        assert_eq!(provider.0.get(), 17);
        assert!(!r.complete);
    }
}
#[test]
fn zero_controls_do_not_open_provider_and_validation_still_runs() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let token = CancellationToken::new();
    token.cancel();
    for stream in [false, true] {
        for (config, reason, evaluations) in [
            (
                Config {
                    max_evaluations: 0,
                    ..Default::default()
                },
                Stop::EvaluationLimit,
                0,
            ),
            (
                Config {
                    max_move_evaluations: Some(0),
                    ..Default::default()
                },
                Stop::MoveEvaluationLimit,
                1,
            ),
            (
                Config {
                    max_steps: Some(0),
                    ..Default::default()
                },
                Stop::StepLimit,
                1,
            ),
            (
                Config {
                    max_scanned_moves: 0,
                    ..Default::default()
                },
                Stop::ScanLimit,
                1,
            ),
            (
                Config {
                    max_time: Some(Duration::ZERO),
                    ..Default::default()
                },
                Stop::TimeLimit,
                0,
            ),
            (
                Config {
                    cancellation: Some(&token),
                    ..Default::default()
                },
                Stop::Cancelled,
                0,
            ),
        ] {
            let mut bomb = |_: &FiniteProblem<i32>,
                            _: &Assignment<i32>,
                            _: u64|
             -> Vec<Move<i32>> { panic!("provider must not open") };
            let r = basic(&p, p.initial_assignment(), stream, config, &mut bomb);
            assert_eq!(r.termination, reason);
            assert_eq!(r.evaluations, evaluations);
        }
    }
    let mut bomb = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| -> Vec<Move<i32>> {
        panic!("provider")
    };
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    assert!(
        construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            Config {
                max_evaluations: 0,
                ..Default::default()
            },
            &mut bomb
        )
        .is_err()
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = b.source::<usize, (usize, Option<i32>)>();
    let other = StreamGraphBuilder::<SimpleScore>::new().build();
    assert!(
        stream_construction_from_with_provider(
            &p,
            other,
            foreign,
            p.initial_assignment(),
            Config {
                max_evaluations: 0,
                ..Default::default()
            },
            &mut bomb
        )
        .is_err()
    );
}
#[test]
fn cancellation_during_scoring_retains_winner_and_early_pick_does_not_prefetch() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    for stream in [false, true] {
        let token = CancellationToken::new();
        let mut provider =
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![change(0, 1), change(0, 2)];
        let config = Config {
            cancellation: Some(&token),
            ..Default::default()
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i32>)>();
            b.score("cancel", s.port(), |row| {
                if row.1.is_some() {
                    token.cancel();
                }
                SimpleScore::new(0)
            })
            .unwrap();
            stream_construction_from_with_provider(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("cancel", vec![0, 1], |v| {
                if v[0].is_some() {
                    token.cancel();
                }
                SimpleScore::new(0)
            })
            .unwrap();
            construction_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(r.termination, Stop::Cancelled);
        assert_eq!(r.evaluations, 2);
        assert_eq!(r.assignment.values(), &[Some(1), None]);
        assert_eq!(r.scanned_moves, 1);
    }
    struct One;
    impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for One {
        fn moves<'a>(
            &'a mut self,
            _: &'a FiniteProblem<i32>,
            _: &'a Assignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
        where
            Move<i32>: 'a,
        {
            Box::new(std::iter::once(change(0, -1)).chain(std::iter::from_fn(
                || -> Option<Move<i32>> { panic!("early pick prefetch") },
            )))
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![-1])]).unwrap();
    for stream in [false, true] {
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config {
                pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
                ..Default::default()
            },
            &mut One,
        );
        assert!(r.complete);
        assert_eq!(r.scanned_moves, 1);
    }
}
#[test]
fn empty_provider_does_not_claim_impossibility() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    for stream in [false, true] {
        let r = basic(
            &p,
            p.initial_assignment(),
            stream,
            Config::default(),
            &mut |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![],
        );
        assert!(!r.complete);
        assert_eq!(r.termination, Stop::NoCandidates);
        assert_eq!(r.evaluations, 1);
    }
}

#[test]
fn list_and_mixed_limits_preserve_partial_work_across_both_engines() {
    let lp = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![])]).unwrap();
    let mp = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap(),
        lp.clone(),
    );
    for stream in [false, true] {
        for config in [
            Config {
                max_move_evaluations: Some(1),
                ..Default::default()
            },
            Config {
                max_scanned_moves: 1,
                ..Default::default()
            },
        ] {
            let expected = if config.max_move_evaluations.is_some() {
                Stop::MoveEvaluationLimit
            } else {
                Stop::ScanLimit
            };
            let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| {
                vec![insert(1, 0, 0), insert(2, 0, 0)]
            };
            let r = lists(&lp, lp.initial_assignment(), stream, config, &mut provider);
            assert_eq!(r.termination, expected);
            assert!(!r.complete);
            assert_eq!(r.assignment.lists(), &[vec![1]]);
            assert_eq!(
                (r.scanned_moves, r.evaluations, r.completed_steps),
                (1, 2, 1)
            );
            let mut provider =
                |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
                    vec![
                        MixedMove {
                            basic: change(0, 1),
                            lists: insert(1, 0, 0),
                        },
                        MixedMove {
                            basic: change(0, 2),
                            lists: insert(2, 0, 0),
                        },
                    ]
                };
            let r = mixed(&mp, stream, config, &mut provider);
            assert_eq!(r.termination, expected);
            assert!(!r.complete);
            assert_eq!(r.assignment.basic().values(), &[Some(1)]);
            assert_eq!(r.assignment.lists().lists(), &[vec![1]]);
            assert_eq!(
                (r.scanned_moves, r.evaluations, r.completed_steps),
                (1, 2, 1)
            );
        }
    }
}
#[test]
fn mixed_atomic_filter_protects_optional_unassignment_and_existing_components() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).allowing_unassigned()]).unwrap(),
        ListProblem::new(vec![10, 20, 30], vec![ListEntity::new(vec![10, 20])]).unwrap(),
    );
    for stream in [false, true] {
        let mut provider = |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
            vec![
                MixedMove {
                    basic: change(0, 1),
                    lists: insert(30, 0, 2),
                }, // Optional None preserved.
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Composite(vec![
                        ListMove::Swap {
                            left: ListPosition {
                                entity: 0,
                                index: 0,
                            },
                            right: ListPosition {
                                entity: 0,
                                index: 1,
                            },
                        },
                        insert(30, 0, 2),
                    ]),
                },
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: insert(30, 0, 1),
                },
            ]
        };
        let r = mixed(&p, stream, Config::default(), &mut provider);
        assert!(r.complete);
        assert_eq!(r.assignment.basic().values(), &[None]);
        assert_eq!(r.assignment.lists().lists(), &[vec![10, 30, 20]]);
        assert_eq!((r.rejected_moves, r.evaluations), (2, 2));
    }
}

#[test]
fn atomic_provider_scans_discard_on_score_scan_and_cancellation_limits() {
    for stream in [false, true] {
        for reason in 0..3 {
            let token = CancellationToken::new();
            let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
            let pulls = Cell::new(0);
            // Cancel from the second raw proposal, after one scored candidate.
            struct Provider<'a> {
                token: &'a CancellationToken,
                pulls: &'a Cell<usize>,
                cancel: bool,
            }
            impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider<'_> {
                fn moves<'a>(
                    &'a mut self,
                    _: &'a FiniteProblem<i32>,
                    _: &'a Assignment<i32>,
                    _: u64,
                ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
                where
                    Move<i32>: 'a,
                {
                    Box::new([1, 2].into_iter().map(|v| {
                        self.pulls.set(self.pulls.get() + 1);
                        if self.cancel && v == 2 {
                            self.token.cancel();
                        }
                        change(0, v)
                    }))
                }
            }
            let mut provider = Provider {
                token: &token,
                pulls: &pulls,
                cancel: reason == 2,
            };
            let c = Config {
                interruption: ConstructionInterruption::DiscardIncompleteStep,
                max_move_evaluations: (reason == 0).then_some(1),
                max_scanned_moves: if reason == 1 { 1 } else { 100 },
                cancellation: Some(&token),
                assert_scores: true,
                ..Default::default()
            };
            let r = basic(&p, p.initial_assignment(), stream, c, &mut provider);
            assert_eq!(r.assignment.values(), &[None]);
            assert_eq!((r.evaluations, r.completed_steps), (2, 0));
            assert_eq!(
                r.termination,
                [Stop::MoveEvaluationLimit, Stop::ScanLimit, Stop::Cancelled][reason]
            );
            assert_eq!(pulls.get(), if reason == 2 { 2 } else { 1 });
        }
        // Provider exhaustion is not probed beyond an exact candidate limit.
        // Deliberate early pick nevertheless completes the step at that limit.
        let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _| vec![change(0, 0)];
        for early in [false, true] {
            let r = basic(
                &p,
                p.initial_assignment(),
                stream,
                Config {
                    interruption: ConstructionInterruption::DiscardIncompleteStep,
                    max_move_evaluations: Some(1),
                    pick_early: if early {
                        ConstructionPickEarly::FirstNonDeterioratingScore
                    } else {
                        ConstructionPickEarly::Never
                    },
                    ..Default::default()
                },
                &mut provider,
            );
            assert_eq!(r.assignment.values(), &[early.then_some(0)]);
            assert_eq!(r.completed_steps, u64::from(early));
        }
    }
}

#[test]
fn provider_optional_no_change_competes_last_in_all_models_and_backends() {
    for stream in [false, true] {
        let c = Config {
            optional: OptionalConstruction::EvaluateUnassigned,
            interruption: ConstructionInterruption::DiscardIncompleteStep,
            assert_scores: true,
            ..Default::default()
        };
        let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned()]).unwrap();
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _| vec![change(0, 1)];
        let r = basic(&p, p.initial_assignment(), stream, c, &mut provider);
        assert_eq!(r.assignment.values(), &[None]);
        assert_eq!(
            (r.evaluations, r.scanned_moves, r.completed_steps),
            (3, 1, 1)
        );
        let l = ListProblem::new(vec![-1], vec![ListEntity::new(vec![])])
            .unwrap()
            .allowing_unassigned();
        let mut provider =
            |_: &ListProblem<i32>, _: &ListAssignment<i32>, _| vec![insert(-1, 0, 0)];
        let r = lists(&l, l.initial_assignment(), stream, c, &mut provider);
        assert!(r.assignment.lists()[0].is_empty());
        assert_eq!(
            (r.evaluations, r.scanned_moves, r.completed_steps),
            (3, 1, 1)
        );
        let l = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
        let m = MixedProblem::new(p, l);
        let mut provider = |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _| {
            vec![MixedMove {
                basic: change(0, 1),
                lists: ListMove::Composite(vec![]),
            }]
        };
        let r = mixed(&m, stream, c, &mut provider);
        assert_eq!(r.assignment.basic().values(), &[None]);
        assert_eq!(r.assignment.lists().lists()[0], vec![1]);
        assert_eq!(
            (r.evaluations, r.scanned_moves, r.completed_steps),
            (3, 1, 1)
        );
    }
}

#[test]
fn dependency_cycles_are_raw_rejections_without_scoring_or_partial_installation() {
    // Value0 depends on1. Appending1 after0 closes a cycle; inserting1 before0
    // makes legal progress. The initial assigned prefix itself has no cycle.
    let lp = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0])])
        .unwrap()
        .with_dependencies(vec![vec![1], vec![]])
        .unwrap();
    let mp = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap(),
        lp.clone(),
    );
    for stream in [false, true] {
        for (offer_valid, cap) in [(false, 100), (true, 100), (true, 1)] {
            let config = Config {
                max_scanned_moves: cap,
                ..Default::default()
            };
            let proposals = || {
                let mut moves = vec![insert(1, 0, 1)];
                if offer_valid {
                    moves.push(insert(1, 0, 0));
                }
                moves
            };
            let mut provider = |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| proposals();
            let list = lists(&lp, lp.initial_assignment(), stream, config, &mut provider);
            let mut provider =
                |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
                    proposals()
                        .into_iter()
                        .map(|lists| MixedMove {
                            basic: Move::Composite(vec![]),
                            lists,
                        })
                        .collect::<Vec<_>>()
                };
            let mixed = mixed(&mp, stream, config, &mut provider);
            let completed = offer_valid && cap > 1;
            let expected = if completed { vec![1, 0] } else { vec![0] };
            assert_eq!(list.assignment.lists(), std::slice::from_ref(&expected));
            assert_eq!(mixed.assignment.lists().lists(), &[expected]);
            assert_eq!(mixed.assignment.basic().values(), &[Some(1)]);
            lp.validate(&list.assignment).unwrap();
            mp.validate(&mixed.assignment).unwrap();
            for (scanned, rejected, evaluations, callbacks, steps, complete, termination) in [
                (
                    list.scanned_moves,
                    list.rejected_moves,
                    list.evaluations,
                    list.constraint_evaluations,
                    list.completed_steps,
                    list.complete,
                    list.termination,
                ),
                (
                    mixed.scanned_moves,
                    mixed.rejected_moves,
                    mixed.evaluations,
                    mixed.constraint_evaluations,
                    mixed.completed_steps,
                    mixed.complete,
                    mixed.termination,
                ),
            ] {
                assert_eq!(rejected, 1);
                assert_eq!(scanned, 1 + u64::from(completed));
                assert_eq!(evaluations, 1 + u64::from(completed));
                assert_eq!(callbacks, 1 + u128::from(completed));
                assert_eq!(steps, u64::from(completed));
                assert_eq!(complete, completed);
                assert_eq!(
                    termination,
                    if completed {
                        Stop::Completed
                    } else if cap == 1 {
                        Stop::ScanLimit
                    } else {
                        Stop::NoCandidates
                    }
                );
            }
        }
    }
}

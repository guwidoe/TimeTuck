//! Original custom-phase dependency preview and budget regressions.
use std::{
    cell::Cell,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn model() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap()
    .with_dependencies(vec![vec![], vec![0]])
    .unwrap()
}
fn bad() -> ListMove<i32> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 0,
            index: 1,
        },
    }
}
fn good() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 1,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn config() -> CustomPhaseConfig<'static> {
    CustomPhaseConfig {
        max_move_evaluations: Some(2),
        ..Default::default()
    }
}
fn visit<A: Clone + Eq + Debug, M, E: Debug>(
    c: &mut CustomPhaseContext<'_, A, M, SimpleScore, E>,
    bad: &M,
    good: &M,
    expected: i64,
    calls: &Cell<usize>,
) -> Result<(), CustomPhaseError<E>> {
    let before = c.assignment().clone();
    let score = c.score();
    let initial_calls = calls.get();
    let mut flawed = 0;
    let mut valid = 0;
    let result = c.evaluate_inspected(
        bad,
        |_, _| panic!("flawed candidate scored"),
        |p| {
            flawed += 1;
            assert!(p.is_flawed());
            assert_eq!(p.inconsistent_values(), [0, 1]);
            assert_ne!(p.assignment(), &before);
            10
        },
    )?;
    assert_eq!(result, Some(10));
    assert_eq!(flawed, 1);
    assert_eq!(calls.get(), initial_calls);
    assert_eq!(c.assignment(), &before);
    assert_eq!(c.score(), score);
    assert_eq!(
        c.evaluate_inspected(
            good,
            |a, s| {
                valid += 1;
                assert_ne!(a, &before);
                assert_eq!(s, SimpleScore::new(expected));
                20
            },
            |_| panic!("sound candidate misclassified")
        )?,
        Some(20)
    );
    assert_eq!(valid, 1);
    assert!(calls.get() > initial_calls);
    assert_eq!(c.assignment(), &before);
    assert_eq!(c.score(), score);
    assert_eq!(
        c.evaluate_inspected(
            bad,
            |_, _| panic!("budget ignored"),
            |_| panic!("budget ignored")
        )?,
        None
    );
    Ok(())
}
fn finish<A: Eq + Debug>(r: CustomPhaseResult<A, SimpleScore>, before: A, calls: usize) {
    assert_eq!(r.assignment, before);
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.work.committed_moves, 0);
    assert_eq!(r.work.constraint_evaluations, calls as u128);
    assert_eq!(r.termination, CustomPhaseTermination::MoveEvaluationLimit);
}
#[test]
fn inspected_trials_route_flaws_without_scoring_and_bound_work_in_all_list_backends() {
    for mode in 0..4 {
        let calls = Cell::new(0);
        if mode < 2 {
            let p = model();
            let before = p.initial_assignment();
            if mode == 0 {
                let mut n = ListConstraintNetwork::new(2, 2);
                n.add_constraint("load", vec![ListScope::Entity(0)], |facts| {
                    calls.set(calls.get() + 1);
                    let ListFact::Entity(v) = &facts[0] else {
                        panic!()
                    };
                    SimpleScore::new(v.len() as i64)
                })
                .unwrap();
                let r = list_custom_phase(&p, &n, before.clone(), config(), |c| {
                    visit(c, &bad(), &good(), 1, &calls)
                })
                .unwrap();
                finish(r, before, calls.get());
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = ListStreamSources::<i32>::new(&mut b);
                b.score("load", source.values.port(), |v| {
                    calls.set(calls.get() + 1);
                    SimpleScore::new(i64::from(v.position.is_some_and(|p| p.entity == 0)))
                })
                .unwrap();
                let r = list_stream_custom_phase(
                    &p,
                    b.build(),
                    source,
                    before.clone(),
                    config(),
                    |c| visit(c, &bad(), &good(), 1, &calls),
                )
                .unwrap();
                finish(r, before, calls.get());
            }
        } else {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 2]).with_initial(0)]).unwrap(),
                model(),
            );
            let before = p.initial_assignment();
            let bad = MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                lists: bad(),
            };
            let good = MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                lists: good(),
            };
            if mode == 2 {
                let mut n = MixedConstraintNetwork::new(1, 2, 2);
                n.add_constraint(
                    "load",
                    vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
                    |facts| {
                        calls.set(calls.get() + 1);
                        let (MixedFact::Basic(v), MixedFact::ListEntity(row)) =
                            (&facts[0], &facts[1])
                        else {
                            panic!()
                        };
                        SimpleScore::new(v.copied().unwrap_or(0) as i64 + row.len() as i64)
                    },
                )
                .unwrap();
                let r = mixed_custom_phase(&p, &n, before.clone(), config(), |c| {
                    visit(c, &bad, &good, 3, &calls)
                })
                .unwrap();
                finish(r, before, calls.get());
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = MixedStreamSources::<i32, i32>::new(&mut b);
                b.score("basic", source.basic.port(), |(_, v)| {
                    calls.set(calls.get() + 1);
                    SimpleScore::new(v.unwrap_or(0) as i64)
                })
                .unwrap();
                b.score("load", source.lists.values.port(), |v| {
                    calls.set(calls.get() + 1);
                    SimpleScore::new(i64::from(v.position.is_some_and(|p| p.entity == 0)))
                })
                .unwrap();
                let r = mixed_stream_custom_phase(
                    &p,
                    b.build(),
                    source,
                    before.clone(),
                    config(),
                    |c| visit(c, &bad, &good, 3, &calls),
                )
                .unwrap();
                finish(r, before, calls.get());
            }
        }
    }
}
#[test]
fn caught_handler_panics_restore_state_and_allow_later_work() {
    for flawed in [false, true] {
        let p = model();
        let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
        let a = p.initial_assignment();
        let r = list_custom_phase(&p, &n, a.clone(), Default::default(), |c| {
            let movement = if flawed { bad() } else { good() };
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _: Option<()> = c
                        .evaluate_inspected(&movement, |_, _| panic!("valid"), |_| panic!("flawed"))
                        .unwrap();
                }))
                .is_err()
            );
            assert_eq!(c.assignment(), &a);
            assert_eq!(c.score(), SimpleScore::ZERO);
            assert_eq!(
                c.evaluate_inspected(&good(), |_, _| 42, |_| panic!())?,
                Some(42)
            );
            c.assert_consistent()?;
            Ok(())
        })
        .unwrap();
        assert_eq!(r.assignment, a);
        assert_eq!(r.work.evaluations, 3);
        assert_eq!(r.work.move_evaluations, 2);
    }
}
#[test]
fn malformed_previews_latch_failure_and_do_not_call_handlers() {
    let p = model();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
    let a = p.initial_assignment();
    let e = list_custom_phase(&p, &n, a.clone(), Default::default(), |c| {
        let invalid = ListMove::Assign {
            value: 99,
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        };
        assert!(
            c.evaluate_inspected(&invalid, |_, _| panic!(), |_| panic!())
                .is_err()
        );
        assert_eq!(c.assignment(), &a);
        assert_eq!(
            c.evaluate_inspected(&good(), |_, _| panic!(), |_| panic!())?,
            None
        );
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(e, CustomPhaseError::IgnoredOperationError));
}
#[test]
fn partial_basic_trials_use_the_valid_handler_and_evaluation_limit() {
    for stream in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
        let a = p.initial_assignment();
        let movement = Move::Change {
            variable: 0,
            value: None,
        };
        let config = CustomPhaseConfig {
            max_evaluations: 2,
            ..Default::default()
        };
        macro_rules! command {
            ($c:ident) => {{
                assert_eq!(
                    $c.evaluate_inspected(
                        &movement,
                        |a, s| {
                            assert_eq!(a.values(), &[None]);
                            assert_eq!(s.structural(), -1);
                            12
                        },
                        |_| panic!()
                    )?,
                    Some(12)
                );
                assert_eq!($c.assignment(), &a);
                assert_eq!(
                    $c.evaluate_inspected(&movement, |_, _| panic!(), |_| panic!())?,
                    None
                );
                Ok(())
            }};
        }
        let r = if stream {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            stream_custom_phase(&p, b.build(), source, a.clone(), config, |c| command!(c)).unwrap()
        } else {
            custom_phase(
                &p,
                &ConstraintNetwork::<_, SimpleScore>::new(1),
                a.clone(),
                config,
                |c| command!(c),
            )
            .unwrap()
        };
        assert_eq!(r.assignment, a);
        assert_eq!(r.work.evaluations, 2);
        assert_eq!(r.work.move_evaluations, 1);
        assert_eq!(r.termination, CustomPhaseTermination::EvaluationLimit);
    }
}

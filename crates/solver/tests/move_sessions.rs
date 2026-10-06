//! Original live-edit, nested rollback and construction failure regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
macro_rules! exercise {
    ($d:ident, $calls:ident, $first:expr, $second:expr, $invalid:expr) => {{
        let initial = $d.assignment().clone();
        let initial_score = $d.score();
        let first = $first;
        let second = $second;
        let invalid = $invalid;
        let (prefix, prefix_score, final_state, final_score);
        {
            let count = $calls.get();
            let mut outer = $d.edit_session().unwrap();
            assert_eq!($calls.get(), count, "snapshot creation must not rescore");
            outer.apply(&first).unwrap();
            prefix = outer.assignment().clone();
            prefix_score = outer.score();
            assert_ne!(prefix, initial);
            assert!(outer.apply(&invalid).is_err());
            assert_eq!(outer.assignment(), &prefix);
            assert_eq!(outer.score(), prefix_score);
            {
                let count = $calls.get();
                let mut nested = outer.nested().unwrap();
                assert_eq!($calls.get(), count);
                nested.apply(&second).unwrap();
                final_state = nested.assignment().clone();
                final_score = nested.score();
                assert_ne!(final_state, prefix);
                let count = $calls.get();
                nested.rollback();
                assert_eq!($calls.get(), count);
            }
            assert_eq!(outer.assignment(), &prefix);
            assert_eq!(outer.score(), prefix_score);
            {
                let mut nested = outer.nested().unwrap();
                nested.apply(&second).unwrap();
                nested.commit();
            }
            assert_eq!(outer.assignment(), &final_state);
            assert_eq!(outer.score(), final_score);
            let count = $calls.get();
            drop(outer);
            assert_eq!(
                $calls.get(),
                count,
                "outer rollback must restore caches without callbacks"
            );
        }
        assert_eq!($d.assignment(), &initial);
        assert_eq!($d.score(), initial_score);
        $d.assert_consistent().unwrap();
        let result = TemporaryMoves::new(&mut $d)
            .construct(
                TemporaryMoveOptions {
                    restored_score: RestoredScorePolicy::CheckFresh,
                    ..Default::default()
                },
                |view| {
                    view.apply(&first).map_err(|_| "first")?;
                    {
                        let mut nested = view.nested().unwrap();
                        nested.apply(&second).unwrap();
                        nested.commit();
                    }
                    assert_eq!(view.assignment(), &final_state);
                    Ok::<_, &str>(())
                },
                |a, s| {
                    assert_eq!(a, &final_state);
                    assert_eq!(s, final_score);
                    Ok("processed")
                },
            )
            .unwrap();
        assert_eq!(result, "processed");
        assert_eq!($d.assignment(), &initial);
        assert_eq!($d.score(), initial_score);
        for failure in [
            TrialFailurePolicy::Rollback,
            TrialFailurePolicy::KeepApplied,
        ] {
            for panic in [false, true] {
                let mut cleanup = $d.edit_session().unwrap();
                let post_called = Cell::new(false);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    cleanup.temporary().construct(
                        TemporaryMoveOptions {
                            failure,
                            ..Default::default()
                        },
                        |view| {
                            view.apply(&first).unwrap();
                            assert_eq!(view.assignment(), &prefix);
                            assert_eq!(view.score(), prefix_score);
                            assert!(view.apply(&invalid).is_err());
                            if panic {
                                panic!("construction panic");
                            }
                            Err::<(), _>("construction failed")
                        },
                        |_, _| {
                            post_called.set(true);
                            Ok::<_, &str>(())
                        },
                    )
                }));
                if panic {
                    assert!(result.is_err());
                } else {
                    assert!(matches!(
                        result,
                        Ok(Err(TemporaryMoveError::Construction("construction failed")))
                    ));
                }
                assert!(!post_called.get());
                let (a, s) = if failure == TrialFailurePolicy::KeepApplied {
                    (&prefix, &prefix_score)
                } else {
                    (&initial, &initial_score)
                };
                assert_eq!(cleanup.assignment(), a);
                assert_eq!(&cleanup.score(), s);
                cleanup.assert_consistent().unwrap();
            }
            let mut cleanup = $d.edit_session().unwrap();
            let result = cleanup.temporary().construct(
                TemporaryMoveOptions {
                    failure,
                    ..Default::default()
                },
                |view| {
                    view.apply(&first).unwrap();
                    view.apply(&second).unwrap();
                    Ok::<_, &str>(())
                },
                |_, _| Err::<(), _>("post failed"),
            );
            assert!(matches!(
                result,
                Err(TemporaryMoveError::Postprocessor("post failed"))
            ));
            let (a, s) = if failure == TrialFailurePolicy::KeepApplied {
                (&final_state, &final_score)
            } else {
                (&initial, &initial_score)
            };
            assert_eq!(cleanup.assignment(), a);
            assert_eq!(&cleanup.score(), s);
            cleanup.assert_consistent().unwrap();
        }
        assert_eq!($d.assignment(), &initial);
        assert_eq!($d.score(), initial_score);
    }};
}
fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap()
}
fn change(value: i32) -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(value),
    }
}
fn list_problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap()
}
fn first_list() -> ListMove<i32> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 1,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn second_list() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 2,
        },
        destination: ListPosition {
            entity: 1,
            index: 1,
        },
    }
}
fn invalid_list() -> ListMove<i32> {
    ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 1,
    }
}
fn score_lists(lists: &[Vec<i32>]) -> SimpleScore {
    SimpleScore::new(
        lists
            .iter()
            .enumerate()
            .map(|(e, l)| {
                l.iter()
                    .enumerate()
                    .map(|(i, v)| (e as i64 + 1) * (i as i64 + 1) * i64::from(*v))
                    .sum::<i64>()
            })
            .sum(),
    )
}
#[test]
fn basic_live_sessions_in_both_backends() {
    let p = basic_problem();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("v", vec![0], |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, calls, change(1), change(2), change(99));
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("v", source.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    exercise!(d, calls, change(1), change(2), change(99));
}
#[test]
fn list_live_sessions_in_both_backends() {
    let p = list_problem();
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "lists",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            calls.set(calls.get() + 1);
            score_lists(
                &facts
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(l) = f else { panic!() };
                        l.to_vec()
                    })
                    .collect::<Vec<_>>(),
            )
        },
    )
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, calls, first_list(), second_list(), invalid_list());
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<i32>::new(&mut b);
    b.score("lists", sources.entities.port(), |(e, l)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(
            l.iter()
                .enumerate()
                .map(|(i, v)| (*e as i64 + 1) * (i as i64 + 1) * i64::from(*v))
                .sum(),
        )
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    exercise!(d, calls, first_list(), second_list(), invalid_list());
}
#[test]
fn mixed_live_sessions_in_both_backends() {
    let p = MixedProblem::new(basic_problem(), list_problem());
    let calls = Cell::new(0);
    let first = MixedMove {
        basic: change(1),
        lists: first_list(),
    };
    let second = MixedMove {
        basic: change(2),
        lists: second_list(),
    };
    // Basic component is valid, list component violates a pin: both must roll back.
    let invalid = MixedMove {
        basic: change(0),
        lists: invalid_list(),
    };
    let mut n = MixedConstraintNetwork::new(1, 2, 4);
    n.add_constraint(
        "mixed",
        vec![
            MixedScope::Basic(0),
            MixedScope::ListEntity(0),
            MixedScope::ListEntity(1),
        ],
        |facts| {
            calls.set(calls.get() + 1);
            let MixedFact::Basic(value) = &facts[0] else {
                panic!()
            };
            SimpleScore::new(i64::from(*value.unwrap()))
                + score_lists(
                    &facts[1..]
                        .iter()
                        .map(|f| {
                            let MixedFact::ListEntity(l) = f else {
                                panic!()
                            };
                            l.to_vec()
                        })
                        .collect::<Vec<_>>(),
                )
        },
    )
    .unwrap();
    let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, calls, first.clone(), second.clone(), invalid.clone());
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::<i32, i32>::new(&mut b);
    b.score("basic", sources.basic.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    b.score("lists", sources.lists.entities.port(), |(e, l)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(
            l.iter()
                .enumerate()
                .map(|(i, v)| (*e as i64 + 1) * (i as i64 + 1) * i64::from(*v))
                .sum(),
        )
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    exercise!(d, calls, first, second, invalid);
}
#[test]
fn scoring_panics_restore_only_the_failed_step_until_outer_rollback() {
    let p = basic_problem();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("panic", vec![0], |v| {
        let v = *v[0].unwrap();
        assert_ne!(v, 2, "score panic");
        SimpleScore::new(i64::from(v))
    })
    .unwrap();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    {
        let mut session = d.edit_session().unwrap();
        session.apply(&change(1)).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| session.apply(&change(2))));
        assert!(result.is_err());
        assert_eq!(session.assignment().values(), &[Some(1)]);
        assert_eq!(session.score(), SimpleScore::new(1));
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::new(0));
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("panic", source.port(), |(_, v)| {
        let v = v.unwrap();
        assert_ne!(v, 2, "score panic");
        SimpleScore::new(i64::from(v))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    {
        let mut session = d.edit_session().unwrap();
        session.apply(&change(1)).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| session.apply(&change(2))));
        assert!(result.is_err());
        assert_eq!(session.assignment().values(), &[Some(1)]);
        assert_eq!(session.score(), SimpleScore::new(1));
        session.assert_consistent().unwrap();
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::new(0));
    d.assert_consistent().unwrap();
}

#[test]
fn stream_session_restores_group_indexes_for_later_incremental_updates() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let counts = b
        .group_collect_with::<_, _, _, _, CountCollector>(source.port(), |(_, v)| (*v, ()))
        .unwrap();
    b.score("balance", &counts, |(_, n)| {
        SimpleScore::new(-(*n as i64).pow(2))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let first = Move::Change {
        variable: 0,
        value: Some(1),
    };
    let second = Move::Change {
        variable: 1,
        value: Some(1),
    };
    for _ in 0..3 {
        {
            let mut outer = d.edit_session().unwrap();
            outer.apply(&first).unwrap();
            assert_eq!(outer.score(), SimpleScore::new(-2));
            {
                let mut inner = outer.nested().unwrap();
                inner.apply(&second).unwrap();
                assert_eq!(inner.score(), SimpleScore::new(-4));
            }
            assert_eq!(outer.score(), SimpleScore::new(-2));
            outer.assert_consistent().unwrap();
            outer.apply(&second).unwrap();
            assert_eq!(outer.score(), SimpleScore::new(-4));
        }
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), SimpleScore::new(-4));
        d.assert_consistent().unwrap();
    }
}

//! Original six-backend temporary processing and recorded replay regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn options(failure: TrialFailurePolicy, fresh: bool) -> TemporaryMoveOptions {
    TemporaryMoveOptions {
        failure,
        restored_score: if fresh {
            RestoredScorePolicy::CheckFresh
        } else {
            RestoredScorePolicy::Cached
        },
    }
}
macro_rules! exercise {
    ($d:ident,$movement:expr,$expected:expr) => {{
        let initial = $d.assignment().clone();
        let score = $d.score();
        let movement = $movement;
        let (_, record) = TemporaryMoves::new(&mut $d)
            .recorded(
                &movement,
                options(TrialFailurePolicy::Rollback, true),
                |a, s| {
                    assert_eq!(a, &$expected);
                    assert_ne!(s, score);
                    Ok::<_, &str>(())
                },
            )
            .unwrap();
        assert_eq!($d.assignment(), &initial);
        assert_eq!($d.score(), score);
        assert_eq!(record.before(), &initial);
        assert_eq!(record.after(), &$expected);
        assert_eq!(
            record.undo(&initial).unwrap_err().direction,
            ReplayDirection::Undo
        );
        for _ in 0..2 {
            $d.apply(record.forward($d.assignment()).unwrap())
                .unwrap()
                .commit();
            assert_eq!($d.assignment(), &$expected);
            assert_eq!(
                record.forward($d.assignment()).unwrap_err().direction,
                ReplayDirection::Forward
            );
            $d.apply(record.undo($d.assignment()).unwrap())
                .unwrap()
                .commit();
            assert_eq!($d.assignment(), &initial);
            assert_eq!($d.score(), score);
            $d.assert_consistent().unwrap();
        }
        for policy in [
            TrialFailurePolicy::Rollback,
            TrialFailurePolicy::KeepApplied,
        ] {
            let result =
                TemporaryMoves::new(&mut $d).process(&movement, options(policy, false), |_, _| {
                    Err::<(), _>("failure")
                });
            assert!(matches!(
                result,
                Err(TemporaryMoveError::Postprocessor("failure"))
            ));
            if policy == TrialFailurePolicy::KeepApplied {
                assert_eq!($d.assignment(), &$expected);
                $d.assert_consistent().unwrap();
                $d.apply(record.undo($d.assignment()).unwrap())
                    .unwrap()
                    .commit();
            } else {
                assert_eq!($d.assignment(), &initial);
                assert_eq!($d.score(), score);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                TemporaryMoves::new(&mut $d).process(
                    &movement,
                    options(policy, false),
                    |_, _| -> Result<(), &str> { panic!("processor panic") },
                )
            }));
            assert!(result.is_err());
            if policy == TrialFailurePolicy::KeepApplied {
                assert_eq!($d.assignment(), &$expected);
                $d.assert_consistent().unwrap();
                $d.apply(record.undo($d.assignment()).unwrap())
                    .unwrap()
                    .commit();
            }
            assert_eq!($d.assignment(), &initial);
            assert_eq!($d.score(), score);
        }
    }};
}
fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2])
            .allowing_unassigned()
            .with_initial(1),
    ])
    .unwrap()
}
fn basic_move() -> Move<i32> {
    Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 0,
            value: Some(2),
        },
        Move::Change {
            variable: 1,
            value: None,
        },
    ])
}
fn list_problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 1, 2, 3, 4],
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4]),
        ],
    )
    .unwrap()
}
fn list_move() -> ListMove<i32> {
    ListMove::Composite(vec![
        ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 1,
            },
            right: ListPosition {
                entity: 1,
                index: 0,
            },
        },
        ListMove::Unassign {
            entity: 1,
            start: 1,
            end: 2,
        },
    ])
}
fn list_expected() -> ListAssignment<i32> {
    ListAssignment::new(vec![vec![0, 3, 2], vec![1]])
}
fn list_score(lists: &[Vec<i32>]) -> SimpleScore {
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
fn basic_scoped_and_stream_processing_and_replay() {
    let p = basic_problem();
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(format!("v{i}"), vec![i], |v| {
            SimpleScore::new(i64::from(v[0].copied().unwrap_or(-10)))
        })
        .unwrap();
    }
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, basic_move(), Assignment::new(vec![Some(2), None]));
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", source.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(-10)))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    exercise!(d, basic_move(), Assignment::new(vec![Some(2), None]));
}
#[test]
fn list_scoped_and_stream_processing_and_replay_preserve_pins() {
    let p = list_problem();
    let mut n = ListConstraintNetwork::new(2, 5);
    n.add_constraint(
        "lists",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |facts| {
            list_score(
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
    exercise!(d, list_move(), list_expected());
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<i32>::new(&mut b);
    b.score("lists", sources.entities.port(), |(e, l)| {
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
    exercise!(d, list_move(), list_expected());
}
#[test]
fn mixed_scoped_and_stream_processing_and_replay_restore_both_components() {
    let p = MixedProblem::new(basic_problem(), list_problem());
    let movement = MixedMove {
        basic: basic_move(),
        lists: list_move(),
    };
    let expected = MixedAssignment::new(Assignment::new(vec![Some(2), None]), list_expected());
    let mut n = MixedConstraintNetwork::new(2, 2, 5);
    n.add_constraint(
        "mixed",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListEntity(0),
            MixedScope::ListEntity(1),
        ],
        |facts| {
            let mut basics = 0;
            let mut lists = vec![];
            for f in facts {
                match f {
                    MixedFact::Basic(v) => basics += i64::from(v.copied().unwrap_or(-10)),
                    MixedFact::ListEntity(l) => lists.push(l.to_vec()),
                    _ => panic!(),
                }
            }
            SimpleScore::new(basics) + list_score(&lists)
        },
    )
    .unwrap();
    let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    exercise!(d, movement.clone(), expected.clone());
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::<i32, i32>::new(&mut b);
    b.score("basic", sources.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(-10)))
    })
    .unwrap();
    b.score("lists", sources.lists.entities.port(), |(e, l)| {
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
    exercise!(d, movement, expected);
}
#[test]
fn fresh_check_recalculates_and_reports_corruption_after_restoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let calls = Cell::new(0);
    let multiplier = Cell::new(1);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("changing external fact", vec![0], |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(multiplier.get() * i64::from(*v[0].unwrap() + 1))
    })
    .unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some(1),
    };
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let start = calls.get();
    TemporaryMoves::new(&mut d)
        .process(
            &movement,
            options(TrialFailurePolicy::Rollback, false),
            |_, _| Ok::<_, ()>(()),
        )
        .unwrap();
    assert_eq!(calls.get() - start, 1);
    let start = calls.get();
    TemporaryMoves::new(&mut d)
        .process(
            &movement,
            options(TrialFailurePolicy::Rollback, true),
            |_, _| Ok::<_, ()>(()),
        )
        .unwrap();
    assert_eq!(calls.get() - start, 2);
    let result = TemporaryMoves::new(&mut d).process(
        &movement,
        options(TrialFailurePolicy::Rollback, true),
        |_, _| {
            multiplier.set(2);
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(TemporaryMoveError::RestoredScore(
            ScoringError::CorruptedConstraint { constraint: 0 }
        ))
    ));
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::new(1));
}
#[test]
fn invalid_moves_do_not_run_postprocessors_or_change_director_state() {
    let p = basic_problem();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut called = false;
    let result = TemporaryMoves::new(&mut d).process(
        &Move::Change {
            variable: 0,
            value: Some(99),
        },
        options(TrialFailurePolicy::KeepApplied, true),
        |_, _| {
            called = true;
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(result, Err(TemporaryMoveError::Apply(_))));
    assert!(!called);
    assert_eq!(d.assignment(), &p.initial_assignment());
}

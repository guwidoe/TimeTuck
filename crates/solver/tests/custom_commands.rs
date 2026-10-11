//! Original command execution regressions; no upstream method credit.
use std::{
    cell::Cell,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[derive(Clone, Copy)]
enum Script {
    Trajectory,
    Budget,
    PanicVisitor,
    IgnoreError,
    PropagateError,
    CommandError,
    ScoringPanic,
}
fn command<A: Clone + PartialEq + Debug, M, E: std::error::Error>(
    c: &mut CustomPhaseContext<'_, A, M, SimpleScore, E>,
    make: impl Fn(&A, i64) -> M,
    script: Script,
) -> Result<(), CustomPhaseError<E>> {
    match script {
        Script::Trajectory => {
            let before = c.assignment().clone();
            let score = c.score();
            assert!(
                c.evaluate(&make(c.assignment(), 2), |a, s| {
                    assert_ne!(a, &before);
                    s
                })?
                .unwrap()
                    > score
            );
            assert_eq!(c.assignment(), &before);
            assert_eq!(c.score(), score);
            c.execute(&make(c.assignment(), 1))?;
            let best = c.best_assignment().clone();
            let best_score = c.best_score();
            c.execute(&make(c.assignment(), 0))?;
            assert!(c.score() < best_score);
            c.evaluate(&make(c.assignment(), 2), |_, s| s)?;
            assert_eq!(c.best_assignment(), &best);
            assert_eq!(c.best_score(), best_score);
            c.execute(&make(c.assignment(), 1))?;
            assert_eq!(c.best_assignment(), &best);
            c.execute(&make(c.assignment(), 2))?;
            c.execute(&make(c.assignment(), 0))?;
        }
        Script::Budget => {
            for (index, value) in [1, 2, 2, 0, 1, 0, 2].into_iter().enumerate() {
                if c.is_terminated() {
                    break;
                }
                if index == 1 {
                    c.evaluate(&make(c.assignment(), value), |_, s| s)?;
                } else {
                    c.execute(&make(c.assignment(), value))?;
                }
            }
        }
        Script::PanicVisitor => {
            let before = c.assignment().clone();
            let score = c.score();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = c.evaluate(&make(c.assignment(), 2), |_, _| panic!("visitor panic"));
                }))
                .is_err()
            );
            assert_eq!(c.assignment(), &before);
            assert_eq!(c.score(), score);
            assert_eq!(c.work().move_evaluations, 1);
            c.assert_consistent()?;
            c.execute(&make(c.assignment(), 1))?;
        }
        Script::ScoringPanic => {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = c.execute(&make(c.assignment(), 1));
                }))
                .is_err()
            );
            assert!(c.is_terminated());
            assert_eq!(c.execute(&make(c.assignment(), 1))?, None);
        }
        Script::IgnoreError => {
            let before = c.assignment().clone();
            let score = c.score();
            assert!(matches!(
                c.execute(&make(c.assignment(), 99)),
                Err(CustomPhaseError::Scoring(_))
            ));
            assert!(c.is_terminated());
            assert_eq!(c.execute(&make(c.assignment(), 1))?, None);
            assert_eq!(c.assignment(), &before);
            assert_eq!(c.score(), score);
        }
        Script::PropagateError => {
            c.execute(&make(c.assignment(), 99))?;
        }
        Script::CommandError => {
            c.execute(&make(c.assignment(), 1))?;
            return Err(CustomPhaseError::Command("abort".into()));
        }
    }
    Ok(())
}
#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<i64>>,
    score: Option<SimpleScore>,
    complete: bool,
    work: CustomPhaseWork,
    reason: CustomPhaseTermination,
}
#[derive(Debug, PartialEq)]
enum Failure {
    Scoring,
    ScoreMismatch,
    Ignored,
    Command,
}
fn failure<E>(e: CustomPhaseError<E>) -> Failure {
    match e {
        CustomPhaseError::Scoring(_) => Failure::Scoring,
        CustomPhaseError::ScoreMismatch => Failure::ScoreMismatch,
        CustomPhaseError::IgnoredOperationError => Failure::Ignored,
        CustomPhaseError::Command(_) => Failure::Command,
    }
}
fn run(
    mode: usize,
    initialized: bool,
    config: CustomPhaseConfig<'_>,
    script: Script,
    events: &mut Vec<(SimpleScore, bool)>,
    hook: impl Fn(),
) -> Result<Outcome, Failure> {
    macro_rules! done {
        ($r:expr,$values:expr) => {{
            let r = $r.map_err(failure)?;
            let values = $values(&r.assignment);
            Ok(Outcome {
                values,
                score: r.score,
                complete: r.complete,
                work: r.work,
                reason: r.termination,
            })
        }};
    }
    if mode < 2 {
        let v = Variable::new(vec![0, 1, 2]);
        let p = FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }]).unwrap();
        let make = |_: &Assignment<i64>, v| Move::Change {
            variable: 0,
            value: Some(v),
        };
        let values = |a: &Assignment<i64>| a.values().to_vec();
        if mode == 0 {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                hook();
                SimpleScore::new(v[0].copied().unwrap_or(0))
            })
            .unwrap();
            return done!(
                custom_phase_with_observer(
                    &p,
                    &n,
                    p.initial_assignment(),
                    config,
                    |c| command(c, make, script),
                    |u| events.push((u.score, u.complete))
                ),
                values
            );
        }
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        b.score("value", s.port(), |(_, v)| {
            hook();
            SimpleScore::new(v.unwrap_or(0))
        })
        .unwrap();
        return done!(
            stream_custom_phase_with_observer(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                config,
                |c| command(c, make, script),
                |u| events.push((u.score, u.complete))
            ),
            values
        );
    }
    let list = || {
        ListProblem::new(
            vec![7i64],
            (0..3)
                .map(|i| {
                    ListEntity::new(if initialized && i == 0 {
                        vec![7]
                    } else {
                        vec![]
                    })
                })
                .collect(),
        )
        .unwrap()
    };
    let list_move = |a: &ListAssignment<i64>, v: i64| {
        let destination = ListPosition {
            entity: v as usize,
            index: 0,
        };
        match a.location(&7) {
            Some(source) => ListMove::Change {
                source,
                destination,
            },
            None => ListMove::Assign {
                value: 7,
                destination,
            },
        }
    };
    if mode < 4 {
        let p = list();
        let values = |a: &ListAssignment<i64>| vec![a.location(&7).map(|p| p.entity as i64)];
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 1);
            n.add_constraint("value", vec![ListScope::Value(0)], |v| {
                hook();
                let ListFact::Value { element, .. } = &v[0] else {
                    panic!()
                };
                SimpleScore::new(element.as_ref().map_or(0, |e| e.position.entity as i64))
            })
            .unwrap();
            return done!(
                list_custom_phase_with_observer(
                    &p,
                    &n,
                    p.initial_assignment(),
                    config,
                    |c| command(c, list_move, script),
                    |u| events.push((u.score, u.complete))
                ),
                values
            );
        }
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("value", s.values.port(), |v| {
            hook();
            SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
        })
        .unwrap();
        return done!(
            list_stream_custom_phase_with_observer(
                &p,
                b.build(),
                s,
                p.initial_assignment(),
                config,
                |c| command(c, list_move, script),
                |u| events.push((u.score, u.complete))
            ),
            values
        );
    }
    let v = Variable::new(vec![0, 1, 2]);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }]).unwrap(),
        list(),
    );
    let make = |a: &MixedAssignment<i64, i64>, v| MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(v),
        },
        lists: list_move(a.lists(), v),
    };
    let values = |a: &MixedAssignment<i64, i64>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity as i64),
        ]
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 3, 1);
        n.add_constraint(
            "value",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                hook();
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                SimpleScore::new(
                    a.copied().unwrap_or(0)
                        + element.as_ref().map_or(0, |e| e.position.entity as i64),
                )
            },
        )
        .unwrap();
        return done!(
            mixed_custom_phase_with_observer(
                &p,
                &n,
                p.initial_assignment(),
                config,
                |c| command(c, make, script),
                |u| events.push((u.score, u.complete))
            ),
            values
        );
    }
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())
        .unwrap();
    b.score("value", &j, |((_, a), v)| {
        hook();
        SimpleScore::new(a.unwrap_or(0) + v.position.map_or(0, |p| p.entity as i64))
    })
    .unwrap();
    done!(
        mixed_stream_custom_phase_with_observer(
            &p,
            b.build(),
            s,
            p.initial_assignment(),
            config,
            |c| command(c, make, script),
            |u| events.push((u.score, u.complete))
        ),
        values
    )
}
#[test]
fn custom_commands_keep_best_commits_while_temporary_and_worsening_moves_change_no_best() {
    for mode in 0..6 {
        let calls = Cell::new(0);
        let mut events = vec![];
        let r = run(
            mode,
            true,
            CustomPhaseConfig::default(),
            Script::Trajectory,
            &mut events,
            || calls.set(calls.get() + 1),
        )
        .unwrap();
        let factor = if mode < 4 { 1 } else { 2 };
        assert_eq!(r.values, vec![Some(2); factor]);
        assert_eq!(r.score, Some(SimpleScore::new(2 * factor as i64)));
        assert!(r.complete);
        assert_eq!(r.reason, CustomPhaseTermination::Completed);
        assert_eq!(r.work.evaluations, 8);
        assert_eq!(r.work.move_evaluations, 7);
        assert_eq!(r.work.committed_moves, 5);
        assert_eq!(r.work.constraint_evaluations, calls.get());
        assert_eq!(
            events,
            vec![
                (SimpleScore::ZERO, true),
                (SimpleScore::new(factor as i64), true),
                (SimpleScore::new(2 * factor as i64), true)
            ]
        );
    }
}
#[test]
fn custom_command_budgets_cap_all_operation_prefixes_without_changing_retained_results() {
    for mode in 0..6 {
        for scores in 0..=10 {
            for moves in 0..=9 {
                let c = CustomPhaseConfig {
                    max_evaluations: scores,
                    max_move_evaluations: Some(moves),
                    ..Default::default()
                };
                let mut events = vec![];
                let r = run(mode, true, c, Script::Budget, &mut events, || {}).unwrap();
                let prefix = run(
                    mode,
                    true,
                    CustomPhaseConfig {
                        max_evaluations: scores.min(moves + 1),
                        max_move_evaluations: None,
                        ..c
                    },
                    Script::Budget,
                    &mut vec![],
                    || {},
                )
                .unwrap();
                assert_eq!(r.values, prefix.values);
                assert_eq!(r.score, prefix.score);
                assert_eq!(r.work, prefix.work);
                assert_eq!(r.work.evaluations, scores.min(moves + 1).min(8));
                assert!(r.work.move_evaluations <= moves);
                assert!(r.complete);
                assert_eq!(events.last().map(|e| e.0), r.score);
                assert!(events.windows(2).all(|w| w[0].0 < w[1].0));
            }
        }
    }
}
#[test]
fn caught_temporary_visitor_panics_rollback_caches_and_consume_their_evaluation() {
    for mode in 0..6 {
        let mut events = vec![];
        let r = run(
            mode,
            true,
            CustomPhaseConfig::default(),
            Script::PanicVisitor,
            &mut events,
            || {},
        )
        .unwrap();
        assert_eq!(r.values, vec![Some(1); if mode < 4 { 1 } else { 2 }]);
        assert_eq!(r.work.evaluations, 3);
        assert_eq!(r.work.move_evaluations, 2);
        assert_eq!(r.work.committed_moves, 1);
        assert_eq!(events.len(), 2);
        assert_eq!(events.last().unwrap().0, r.score.unwrap());
    }
}
#[test]
fn command_errors_and_ignored_scoring_failures_never_report_success() {
    for mode in 0..6 {
        for (script, expected, count) in [
            (Script::IgnoreError, Failure::Ignored, 1),
            (Script::PropagateError, Failure::Scoring, 1),
            (Script::CommandError, Failure::Command, 2),
        ] {
            let mut events = vec![];
            assert_eq!(
                run(
                    mode,
                    true,
                    CustomPhaseConfig::default(),
                    script,
                    &mut events,
                    || {}
                ),
                Err(expected)
            );
            assert_eq!(events.len(), count);
        }
    }
}
#[test]
fn cancellation_and_zero_time_retain_initial_or_committed_prefixes() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        token.cancel();
        let c = CustomPhaseConfig {
            cancellation: Some(&token),
            ..Default::default()
        };
        let r = run(mode, true, c, Script::Trajectory, &mut vec![], || {
            panic!("pre-cancel scored")
        })
        .unwrap();
        assert_eq!(r.score, None);
        assert_eq!(r.work, CustomPhaseWork::default());
        assert_eq!(r.reason, CustomPhaseTermination::Cancelled);
        let r = run(
            mode,
            true,
            CustomPhaseConfig {
                max_evaluations: 0,
                ..c
            },
            Script::Trajectory,
            &mut vec![],
            || panic!("zero scored"),
        )
        .unwrap();
        assert_eq!(r.reason, CustomPhaseTermination::EvaluationLimit);
        let r = run(
            mode,
            true,
            CustomPhaseConfig {
                max_time: Some(std::time::Duration::ZERO),
                ..Default::default()
            },
            Script::Trajectory,
            &mut vec![],
            || {},
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.reason, CustomPhaseTermination::TimeLimit);
        let token = CancellationToken::new();
        let calls = Cell::new(0);
        let r = run(
            mode,
            true,
            CustomPhaseConfig {
                cancellation: Some(&token),
                ..Default::default()
            },
            Script::Budget,
            &mut vec![],
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    token.cancel();
                }
            },
        )
        .unwrap();
        assert_eq!(r.reason, CustomPhaseTermination::Cancelled);
        assert_eq!(r.work.move_evaluations, 1);
        assert_eq!(r.values, vec![Some(1); if mode < 4 { 1 } else { 2 }]);
    }
}

#[test]
fn caught_scoring_panics_invalidate_the_command_instead_of_underreporting_work() {
    for mode in 0..6 {
        let calls = Cell::new(0);
        assert_eq!(
            run(
                mode,
                true,
                CustomPhaseConfig::default(),
                Script::ScoringPanic,
                &mut vec![],
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 2 {
                        panic!("scoring panic");
                    }
                }
            ),
            Err(Failure::Ignored)
        );
    }
}
#[test]
fn custom_commands_can_initialize_partial_models_without_promoting_temporary_completion() {
    for mode in 0..6 {
        let mut events = vec![];
        let r = run(
            mode,
            false,
            CustomPhaseConfig::default(),
            Script::Trajectory,
            &mut events,
            || {},
        )
        .unwrap();
        let count = if mode < 4 { 1 } else { 2 };
        assert!(!events[0].1);
        assert_eq!(events[0].0.structural(), -(count as i64));
        assert_eq!(events.len(), 3);
        assert!(events[1].1);
        assert!(events[2].1);
        assert!(r.complete);
        assert_eq!(r.values, vec![Some(2); count]);
        assert_eq!(r.work.evaluations, 8);
    }
}

#[test]
fn custom_phase_input_and_source_validation_precedes_zero_work_or_cancellation() {
    let token = CancellationToken::new();
    token.cancel();
    for c in [
        CustomPhaseConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        CustomPhaseConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
    ] {
        let p = FiniteProblem::new(vec![Variable::new(vec![1i64])]).unwrap();
        let l = ListProblem::new(vec![7i64], vec![ListEntity::new(vec![])]).unwrap();
        assert!(
            custom_phase(
                &p,
                &ConstraintNetwork::<_, SimpleScore>::new(2),
                p.initial_assignment(),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        assert!(
            custom_phase(
                &p,
                &ConstraintNetwork::<_, SimpleScore>::new(1),
                Assignment::new(vec![]),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        assert!(
            list_custom_phase(
                &l,
                &ListConstraintNetwork::<_, SimpleScore>::new(2, 1),
                l.initial_assignment(),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        assert!(
            list_custom_phase(
                &l,
                &ListConstraintNetwork::<_, SimpleScore>::new(1, 1),
                ListAssignment::new(vec![]),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        let p = MixedProblem::new(p, l);
        assert!(
            mixed_custom_phase(
                &p,
                &MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 1, 1),
                p.initial_assignment(),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        assert!(
            mixed_custom_phase(
                &p,
                &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1),
                MixedAssignment::new(Assignment::new(vec![]), p.lists().initial_assignment()),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        let mut other = StreamGraphBuilder::<SimpleScore>::new();
        let source = other.source::<usize, (usize, Option<i64>)>();
        assert!(
            stream_custom_phase(
                p.basic(),
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.basic().initial_assignment(),
                c,
                |_| panic!("foreign source")
            )
            .is_err()
        );
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        assert!(
            stream_custom_phase(
                p.basic(),
                b.build(),
                source,
                Assignment::new(vec![]),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        let source = ListStreamSources::new(&mut other);
        assert!(
            list_stream_custom_phase(
                p.lists(),
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.lists().initial_assignment(),
                c,
                |_| panic!("foreign source")
            )
            .is_err()
        );
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = ListStreamSources::new(&mut b);
        assert!(
            list_stream_custom_phase(
                p.lists(),
                b.build(),
                source,
                ListAssignment::new(vec![]),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
        let source = MixedStreamSources::new(&mut other);
        assert!(
            mixed_stream_custom_phase(
                &p,
                StreamGraphBuilder::<SimpleScore>::new().build(),
                source,
                p.initial_assignment(),
                c,
                |_| panic!("foreign source")
            )
            .is_err()
        );
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = MixedStreamSources::new(&mut b);
        assert!(
            mixed_stream_custom_phase(
                &p,
                b.build(),
                source,
                MixedAssignment::new(Assignment::new(vec![]), p.lists().initial_assignment()),
                c,
                |_| panic!("invalid command")
            )
            .is_err()
        );
    }
}

//! Original failure, unwind and quota contracts for independently rescored trials.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Context<'a> = CustomPhaseContext<'a, Assignment<i64>, Move<i64>, SimpleScore, ScoringError>;
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap()
}
fn network() -> ConstraintNetwork<'static, i64, SimpleScore> {
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap())
    })
    .unwrap();
    n
}
fn movement() -> Move<i64> {
    Move::Change {
        variable: 0,
        value: Some(1),
    }
}
fn restored(c: &Context<'_>) {
    assert_eq!(c.assignment().values(), &[Some(0)]);
    assert_eq!(c.score(), SimpleScore::ZERO);
    assert_eq!(c.best_score(), SimpleScore::ZERO);
    assert_eq!(c.work().committed_moves, 0);
}
#[test]
fn full_trial_requires_room_for_both_scores_before_installing_the_move() {
    let p = problem();
    let n = network();
    let calls = Cell::new(0);
    for limit in [1, 2, 3] {
        calls.set(0);
        let r = custom_phase(
            &p,
            &n,
            p.initial_assignment(),
            CustomPhaseConfig {
                max_evaluations: limit,
                ..Default::default()
            },
            |c| {
                let trial = c.evaluate_structured_with_score(
                    &movement(),
                    |a| {
                        calls.set(calls.get() + 1);
                        n.calculate(&p, a).map(|s| (s, 1))
                    },
                    |_, s| s,
                    |_, _| panic!("sound trial"),
                )?;
                assert_eq!(trial, (limit == 3).then_some(SimpleScore::new(1)));
                restored(c);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls.get(), usize::from(limit == 3));
        assert_eq!(r.work.evaluations, if limit == 3 { 3 } else { 1 });
        assert_eq!(r.work.move_evaluations, u64::from(limit == 3));
        assert_eq!(r.termination, CustomPhaseTermination::EvaluationLimit);
    }
}
#[test]
fn mismatching_or_failed_full_scores_roll_back_and_cannot_be_ignored() {
    let p = problem();
    let n = network();
    for mismatch in [false, true] {
        let error = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
            let result = c.evaluate_structured_with_score(
                &movement(),
                |_| {
                    if mismatch {
                        Ok((SimpleScore::new(2), 1))
                    } else {
                        Err(ScoringError::CorruptedTotal)
                    }
                },
                |_, _| panic!("failed score cannot reach a handler"),
                |_, _| panic!("failed score cannot reach a handler"),
            );
            if mismatch {
                assert!(matches!(result, Err(CustomPhaseError::ScoreMismatch)));
            } else {
                assert!(matches!(
                    result,
                    Err(CustomPhaseError::Scoring(ScoringError::CorruptedTotal))
                ));
            }
            restored(c);
            assert!(c.is_terminated());
            assert!(c.execute(&movement())?.is_none());
            assert_eq!(c.work().evaluations, 3);
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, CustomPhaseError::IgnoredOperationError));
    }
}
#[test]
fn caught_full_scorer_panics_invalidate_the_phase_but_handler_panics_can_resume() {
    let p = problem();
    let n = network();
    for scorer_panics in [false, true] {
        let result = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
            let panic = catch_unwind(AssertUnwindSafe(|| {
                c.evaluate_structured_with_score(
                    &movement(),
                    |a| {
                        assert_eq!(a.values(), &[Some(1)]);
                        assert!(!scorer_panics, "full scorer panic");
                        n.calculate(&p, a).map(|s| (s, 1))
                    },
                    |_, _| panic!("handler panic"),
                    |_, _| panic!("sound trial"),
                )
            }));
            assert!(panic.is_err());
            restored(c);
            assert_eq!(c.work().evaluations, 3);
            assert_eq!(c.is_terminated(), scorer_panics);
            if !scorer_panics {
                assert_eq!(c.execute(&movement())?, Some(SimpleScore::new(1)));
            }
            Ok(())
        });
        if scorer_panics {
            assert!(matches!(
                result,
                Err(CustomPhaseError::IgnoredOperationError)
            ));
        } else {
            let r = result.unwrap();
            assert_eq!(r.score, Some(SimpleScore::new(1)));
            assert_eq!(r.work.evaluations, 4);
        }
    }
}
#[test]
fn invalid_initial_trial_never_charges_or_invokes_the_full_rescorer() {
    let p = problem();
    let n = network();
    let r = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
        assert!(
            c.evaluate_structured_with_score(
                &Move::Change {
                    variable: 0,
                    value: Some(9)
                },
                |_| panic!("invalid domain"),
                |_, _| (),
                |_, _| ()
            )
            .is_err()
        );
        restored(c);
        assert_eq!(c.work().evaluations, 2);
        assert_eq!(c.work().move_evaluations, 1);
        Ok(())
    });
    assert!(matches!(r, Err(CustomPhaseError::IgnoredOperationError)));
}
#[test]
fn independent_rescoring_reaches_shared_phase_score_termination() {
    let p = problem();
    let n = network();
    let condition = SearchTermination::Condition(SearchLimit::ScoreEvaluations(3));
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    let calls = Cell::new(0);
    let mut command = |_: usize, c: &mut Context<'_>| {
        assert_eq!(
            c.evaluate_structured_with_score(
                &movement(),
                |a| {
                    calls.set(calls.get() + 1);
                    n.calculate(&p, a).map(|s| (s, 1))
                },
                |_, s| s,
                |_, _| panic!("sound trial")
            )?,
            Some(SimpleScore::new(1))
        );
        restored(c);
        assert!(c.is_terminated());
        assert!(c.execute(&movement())?.is_none());
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.work.constraint_evaluations, 3);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}

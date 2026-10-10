//! Fresh phase-output scoring through each backend's initial scoring kernel.
use super::*;

pub(super) fn verify<A: Clone, S: AdditiveScore, E>(
    index: usize,
    kind: SolverPhaseKind,
    assignment: &A,
    actual: Option<&S>,
    phase: &mut impl FnMut(
        usize,
        Option<A>,
        PhaseRequest<'_, S>,
        &mut SolveControl<'_>,
        &mut PlanObservation<'_, A, S>,
    ) -> Result<PhaseOutput<A, S>, SolverError<E>>,
) -> Result<(), SolverError<E>> {
    let Some(actual) = actual else { return Ok(()) };
    // Reuse the no-command initialization kernel in all nine backends. Isolate
    // counters, stagnation and event state. This diagnostic has no search budget
    // of its own; elapsed time is still observed by the real solve's next poll.
    let mut clock = || Duration::ZERO;
    let mut time = SolveControl {
        move_counter: None,
        lifecycle: Default::default(),
        condition: None,
        condition_expired: false,
        limit: None,
        clock: &mut clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: None,
        steps: StepProgress::default(),
        construction_steps: 0,
        started: false,
        steps_expired: false,
    };
    let mut observation = PlanObservation::new(None);
    observation.initial_only = true;
    let expected = phase(
        index,
        Some(assignment.clone()),
        PhaseRequest::Custom(
            CustomPhaseConfig {
                max_evaluations: 1,
                max_move_evaluations: Some(0),
                max_time: None,
                cancellation: None,
            },
            None,
            false,
        ),
        &mut time,
        &mut observation,
    )
    .map_err(|error| match error {
        SolverError::Custom {
            error: CustomPhaseError::Scoring(error),
            ..
        }
        | SolverError::Phase { error, .. } => {
            if kind == SolverPhaseKind::Initial {
                SolverError::InitialScoring { error }
            } else {
                SolverError::Phase {
                    phase: index,
                    error,
                }
            }
        }
        other => other,
    })?
    .score
    .ok_or_else(|| SolverError::PhaseScoreCheckFailed {
        phase: index,
        kind,
        reason: "fresh scoring did not return a score".into(),
    })?;
    if &expected != actual {
        return Err(SolverError::PhaseScoreMismatch {
            phase: index,
            kind,
            actual: format!("{actual:?}"),
            expected: format!("{expected:?}"),
        });
    }
    Ok(())
}

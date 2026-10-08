//! Original deterministic shared-deadline check for independent phase scoring.
use super::*;
use std::cell::Cell;
#[test]
fn phase_check_time_consumes_shared_deadline_without_consuming_evaluations() {
    let elapsed = Cell::new(Duration::ZERO);
    let mut clock = || elapsed.get();
    let mut check = |index, kind, _: &usize| {
        assert_eq!((index, kind), (0, SolverPhaseKind::Construction));
        elapsed.set(Duration::from_secs(2));
        Ok(Some(SimpleScore::ZERO))
    };
    let mut observation = PlanObservation::new(PlanCallback::extensions(None, Some(&mut check)));
    let mut entered = vec![];
    let result: Result<_, SolverError<()>> = run_with_clock_observed(
        SolverLimits {
            max_time: Some(Duration::from_secs(1)),
            ..Default::default()
        },
        &[
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(Default::default()),
        ],
        || InitialState {
            assignment: 0usize,
            complete: true,
            movable: true,
        },
        &mut observation,
        |phase, _, _, _, _| {
            entered.push(phase);
            Ok(PhaseOutput {
                assignment: Some(0),
                score: Some(SimpleScore::ZERO),
                complete: true,
                work: SolverWork {
                    evaluations: 1,
                    ..Default::default()
                },
                termination: PhaseTermination::Construction(ConstructionTermination::Completed),
            })
        },
        &mut clock,
    );
    let result = result.unwrap();
    assert_eq!(entered, [0]);
    assert_eq!(result.termination, SolverTermination::TimeLimit);
    assert_eq!(result.work.evaluations, 1);
    assert_eq!(result.work.move_evaluations, 0);
}
#[test]
fn score_without_new_assignment_checks_the_retained_phase_entry() {
    let mut checked = vec![];
    let mut check = |index, kind, a: &usize| {
        checked.push((index, kind, *a));
        Ok(Some(SimpleScore::ZERO))
    };
    let mut observation = PlanObservation::new(PlanCallback::extensions(None, Some(&mut check)));
    let mut clock = || Duration::ZERO;
    let phases = [
        SolverPhase::Construction(Default::default()),
        SolverPhase::Exact(Default::default()),
        SolverPhase::LocalSearch(Default::default()),
    ];
    let result: Result<_, SolverError<()>> = run_with_clock_observed(
        SolverLimits::default(),
        &phases,
        || InitialState {
            assignment: 0usize,
            complete: true,
            movable: true,
        },
        &mut observation,
        |phase, _, _, _, _| {
            Ok(PhaseOutput {
                assignment: (phase != 1).then_some(7),
                score: Some(SimpleScore::ZERO),
                complete: true,
                work: SolverWork {
                    evaluations: 1,
                    ..Default::default()
                },
                termination: match phase {
                    0 => PhaseTermination::Construction(ConstructionTermination::Completed),
                    1 => PhaseTermination::Exact(ExactPhaseTermination::CompletionExhausted),
                    _ => PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit),
                },
            })
        },
        &mut clock,
    );
    assert_eq!(result.unwrap().assignment, 7);
    assert_eq!(
        checked,
        [
            (0, SolverPhaseKind::Construction, 7),
            (1, SolverPhaseKind::Exact, 7),
            (2, SolverPhaseKind::LocalSearch, 7)
        ]
    );
}

#[test]
fn automatic_phase_check_time_counts_without_advancing_search_or_work() {
    let elapsed = Cell::new(Duration::ZERO);
    let mut clock = || elapsed.get();
    let mut actual = 0;
    let mut diagnostics = 0;
    let result: Result<_, SolverError<()>> = run_with_clock_observed(
        SolverLimits {
            assert_phase_scores: true,
            max_evaluations: 1,
            max_time: Some(Duration::from_secs(1)),
            ..Default::default()
        },
        &[
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(Default::default()),
        ],
        || InitialState {
            assignment: 0usize,
            complete: true,
            movable: true,
        },
        &mut PlanObservation::new(None),
        |index, a, request, time, observation| {
            assert_eq!(index, 0);
            if observation.initial_only {
                diagnostics += 1;
                let PhaseRequest::Custom(c, _, _) = request else {
                    panic!("diagnostic kernel")
                };
                assert_eq!(c.max_evaluations, 1);
                assert_eq!(c.max_move_evaluations, Some(0));
                assert!(c.cancellation.is_none());
                assert!(time.monitor().is_none());
                elapsed.set(Duration::from_secs(2));
            } else {
                actual += 1;
            }
            Ok(PhaseOutput {
                assignment: a,
                score: Some(SimpleScore::ZERO),
                complete: true,
                work: SolverWork {
                    evaluations: 1,
                    ..Default::default()
                },
                termination: PhaseTermination::Construction(ConstructionTermination::Completed),
            })
        },
        &mut clock,
    );
    let r = result.unwrap();
    assert_eq!((actual, diagnostics), (1, 1));
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.work.completed_steps, 0);
    assert_eq!(r.work.construction_steps, 0);
    assert_eq!(r.phases.len(), 1);
}

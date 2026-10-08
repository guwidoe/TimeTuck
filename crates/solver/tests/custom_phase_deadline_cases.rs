// Adapted from DefaultCustomPhaseTest, TestdataSolution and TestdataConstraintProvider.
// Timefold Community, Apache-2.0. See compat/provenance.json for provenance.
use std::time::{Duration, Instant};
use timetuck::*;

fn exercise(solver_limit: bool) {
    let run_time = Duration::from_millis(10);
    let problem = FiniteProblem::new(
        (0..2)
            .map(|i| Variable::new(vec![0, 1]).with_initial(i))
            .collect(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(2);
    for i in 0..2 {
        network
            .add_constraint(format!("Always penalize {i}"), vec![i], |values| {
                SimpleScore::new(if values[0].is_some() { -1 } else { 0 })
            })
            .unwrap();
    }
    let plan = SolverSequence {
        limits: SolverLimits {
            max_time: solver_limit.then_some(run_time),
            ..Default::default()
        },
        phases: vec![SolverPhase::Custom(CustomPhaseConfig {
            max_time: (!solver_limit).then_some(run_time),
            ..Default::default()
        })],
    };
    let start = Instant::now();
    let mut entered = false;
    let result = solve_with_commands(&problem, &network, plan, |_, context| {
        entered = true;
        while !context.is_terminated() {
            // Fail a broken cooperative deadline instead of hanging the suite.
            assert!(start.elapsed() < Duration::from_secs(5));
            std::hint::spin_loop();
        }
        Ok(())
    })
    .unwrap();
    assert!(start.elapsed() >= run_time);
    // Original contract only asserts elapsed time. These native checks additionally
    // check the phase termination when entered. A delayed invocation may exhaust
    // either deadline before the callback starts.
    if entered {
        assert_eq!(
            result.phases[0].termination,
            PhaseTermination::Custom(CustomPhaseTermination::TimeLimit)
        );
    }
    assert_eq!(result.assignment.values(), &[Some(0), Some(1)]);
    assert_eq!(result.score, Some(SimpleScore::new(-2)));
}
#[test]
fn solver_termination() {
    exercise(true);
}
#[test]
fn phase_termination() {
    exercise(false);
}

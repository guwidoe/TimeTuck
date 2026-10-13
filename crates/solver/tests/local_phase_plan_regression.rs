//! Original runtime effects of chained phase settings and inherited budgets.
use std::num::NonZeroU64;
use timetuck::*;
#[test]
fn chained_tabu_phase_reaches_feasibility_and_stops_actual_solve() {
    let p =
        FiniteProblem::new(vec![Variable::new(vec![0_i32, 1, 2, 3]).with_initial(0); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("feasibility", vec![0, 1], |values| {
        HardSoftScore::new(i64::from(*values[0].unwrap() + *values[1].unwrap()) - 6, 0)
    })
    .unwrap();
    let plan = LocalSearchPhasePlan::new(LocalSearchConfig {
        max_steps: 100,
        max_evaluations: 200,
        ..Default::default()
    })
    .with_tabu_search(1, true)
    .with_feasible_stop(true)
    .with_accepted_candidate_limit(NonZeroU64::new(5));
    let result = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![plan.into_phase()],
            ..Default::default()
        },
        SolverExtensions::default(),
    )
    .unwrap();
    assert_eq!(result.assignment.values(), &[Some(3), Some(3)]);
    assert_eq!(result.score, Some(HardSoftScore::new(0, 0)));
    assert_eq!(
        result.phases[0].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::FeasibleScore)
    );
    assert!(result.work.completed_steps < 100);
}
#[test]
fn accepted_scan_cap_changes_selected_move_and_can_be_cleared_without_losing_policy() {
    let p =
        FiniteProblem::new(vec![Variable::new(vec![1_i32, 2, 3, 4, 5]).with_initial(1)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("profit", vec![0], |values| {
        HardSoftScore::new(-1, i64::from(*values[0].unwrap()))
    })
    .unwrap();
    let run = |cap| {
        let plan = LocalSearchPhasePlan::new(LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 50,
            ..Default::default()
        })
        .with_tabu_search(1, true)
        .with_feasible_stop(false)
        .with_accepted_candidate_limit(NonZeroU64::new(5))
        .with_accepted_candidate_limit(cap);
        assert!(matches!(
            plan.config().acceptance,
            Acceptance::VariableTabu {
                tenure: 1,
                aspiration: true
            }
        ));
        local_search(&p, &n, plan.into_config()).unwrap()
    };
    let limited = run(NonZeroU64::new(1));
    let uncapped = run(None);
    assert_eq!(limited.best.unwrap().assignment.values(), &[Some(2)]);
    assert_eq!(uncapped.best.unwrap().assignment.values(), &[Some(5)]);
    assert!(limited.evaluations < uncapped.evaluations);
}

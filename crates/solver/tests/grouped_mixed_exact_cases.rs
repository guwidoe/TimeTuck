// Complete pinned Community BruteForceTest mixed methods, Apache-2.0.
// Native unbounded Cartesian search preserves every criterion; no Java trace parity.
#[path = "fixtures/grouped_mixed_exact_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn run() -> MixedBranchAndBoundResult<i32, i32, SimpleScore> {
    let mut steps = 0;
    let result = GroupedMixedExactSolver::default()
        .solve(
            &problem(),
            &network(),
            Some(&mut |event| match event {
                SolverLifecycleEvent::PhaseStarted { kind, .. }
                | SolverLifecycleEvent::PhaseEnded { kind, .. } => {
                    assert_eq!(kind, SolverPhaseKind::Exact)
                }
                SolverLifecycleEvent::StepStarted { kind, step, .. } => {
                    assert_eq!(kind, SolverPhaseKind::Exact, "wrong phase was started");
                    assert!(
                        step < 60,
                        "The exhaustive search phase was not ended after 60 steps."
                    );
                    assert_eq!(step, steps);
                    steps += 1;
                }
                SolverLifecycleEvent::StepEnded { completed, .. } => assert!(completed),
                _ => {}
            }),
        )
        .unwrap();
    // Nonvacuous native expansion count and full exhaustive leaf proof.
    assert_eq!(steps, 33);
    assert_eq!(result.expanded_nodes, steps);
    assert_eq!(result.complete_evaluations, 1944);
    assert_eq!(result.pruned_branches, 0);
    assert!(result.is_optimal());
    result
}
#[test]
fn solve_with_mixed_model() {
    let result = run();
    let best = result.best.unwrap();
    let unassigned_values: Vec<_> = (0..3)
        .filter(|v| {
            !best
                .assignment
                .lists()
                .lists()
                .iter()
                .any(|row| row.contains(v))
        })
        .collect();
    assert!(unassigned_values.is_empty());
    let unassigned_other_values: Vec<_> = (0..2)
        .filter(|e| {
            best.assignment.basic().values()[2 * e].is_none()
                || best.assignment.basic().values()[2 * e + 1].is_none()
        })
        .collect();
    assert!(unassigned_other_values.is_empty());
    assert_eq!(best.score, SimpleScore::new(7));
}
#[test]
fn solve_with_mixed_model_multiple_entities() {
    // Separate native list and basic components represent the source's two
    // concrete entity families, with two basic fields per other entity.
    let result = run();
    let best = result.best.unwrap();
    assert_eq!(
        best.assignment
            .lists()
            .lists()
            .iter()
            .map(Vec::len)
            .sum::<usize>(),
        problem().lists().values().len()
    );
    let unassigned_other_values: Vec<_> = (0..2)
        .filter(|e| {
            best.assignment.basic().values()[2 * e].is_none()
                || best.assignment.basic().values()[2 * e + 1].is_none()
        })
        .collect();
    assert!(unassigned_other_values.is_empty());
    assert_eq!(best.score, SimpleScore::new(7));
}

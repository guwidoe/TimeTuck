// Adapted from Community DefaultSolverTest and its test utilities.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

type ObservedSolve = (
    SolverResult<Assignment<i32>, SimpleScore>,
    Vec<(SolverPhaseKind, SimpleScore)>,
);
fn run(variables: Vec<Variable<i32>>) -> ObservedSolve {
    let p = FiniteProblem::new(variables).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(p.variables().len());
    let plan = SolverSequence {
        limits: SolverLimits::default(),
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
    };
    let mut events = vec![];
    let result = solve_with_commands_and_events(
        &p,
        &n,
        plan,
        |_, _| panic!("all phases must be skipped without movable entities"),
        |e| match e {
            SolverEvent::Best(u) => events.push((u.kind, u.score)),
            SolverEvent::FirstInitialized { .. } => panic!("no phase entered"),
        },
    )
    .unwrap();
    (result, events)
}
#[test]
fn solve_empty_entity_list() {
    // Upstream declares two unused values; native domains live on variables.
    let (r, events) = run(vec![]);
    assert!(r.assignment.values().iter().all(Option::is_some));
    assert_eq!(events, [(SolverPhaseKind::Initial, SimpleScore::ZERO)]);
}
#[test]
fn solve_empty_entity_list_and_empty_value_list() {
    let (r, events) = run(vec![]);
    assert!(r.score.is_some());
    assert!(!events.is_empty());
}
#[test]
fn solve_pinned_entity_list() {
    let (r, events) = run(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]).with_initial(2).with_pinned(true),
    ]);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert!(!events.is_empty());
}

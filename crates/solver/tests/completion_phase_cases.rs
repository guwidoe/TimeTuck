// Adapted from DefaultExhaustiveSearchPhaseTest and test-only domain/helpers.
// Timefold Community, Apache-2.0; see compat/provenance.json.
use timetuck::*;
type Value = &'static str;
fn config() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Complete,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn basic(p: &FiniteProblem<Value>) -> SolverResult<Assignment<Value>, SimpleScore> {
    let n = ConstraintNetwork::new(p.variables().len());
    let mut events = 0;
    let r = solve_with_events(p, &n, config(), |e| {
        if matches!(e, SolverEvent::Best(_)) {
            events += 1
        }
    })
    .unwrap();
    assert!(events > 0); // PlannerTestUtils.solve's default event assertion.
    r
}
fn lists(
    p: &ListProblem<Value>,
    expect_event: bool,
) -> SolverResult<ListAssignment<Value>, SimpleScore> {
    let n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
    let mut events = 0;
    let r = solve_list_with_events(p, &n, config(), |e| {
        if matches!(e, SolverEvent::Best(_)) {
            events += 1
        }
    })
    .unwrap();
    assert_eq!(events > 0, expect_event);
    r
}
fn variable() -> Variable<Value> {
    Variable::new(vec!["v1", "v2", "v3"])
}
fn basic_entities(a: &Assignment<Value>) -> Vec<(Value, Option<Value>)> {
    ["e1", "e2", "e3"]
        .into_iter()
        .zip(a.values().iter().copied())
        .collect()
}
fn list_entities(a: &ListAssignment<Value>) -> Vec<(Value, Vec<Value>)> {
    ["e1", "e2", "e3"]
        .into_iter()
        .zip(a.lists().iter().cloned())
        .collect()
}
#[test]
fn solve_basic_variable_with_initialized_entities() {
    let p = FiniteProblem::new(vec![
        variable(),
        variable().with_initial("v2"),
        variable().with_initial("v1"),
    ])
    .unwrap();
    let r = basic(&p);
    let entities = basic_entities(&r.assignment);
    assert_eq!(entities[0].0, "e1");
    assert!(entities[0].1.is_some());
    assert_eq!(entities[1], ("e2", Some("v2")));
    assert_eq!(entities[2], ("e3", Some("v1")));
}
#[test]
fn solve_list_variable_with_initialized_entities() {
    let p = ListProblem::new(
        vec!["v1", "v2", "v3"],
        vec![
            ListEntity::new(vec![]),
            ListEntity::new(vec!["v2"]),
            ListEntity::new(vec!["v1"]),
        ],
    )
    .unwrap();
    let r = lists(&p, true);
    assert_eq!(
        list_entities(&r.assignment),
        [
            ("e1", vec![p.values()[2]]),
            ("e2", vec![p.values()[1]]),
            ("e3", vec![p.values()[0]])
        ]
    );
}
#[test]
fn solve_basic_with_pinned_entities() {
    let p = FiniteProblem::new(vec![
        variable(),
        variable().with_initial("v2"),
        variable().with_initial("v3").with_pinned(true),
    ])
    .unwrap();
    let r = basic(&p);
    let entities = basic_entities(&r.assignment);
    assert_eq!(entities[0].0, "e1");
    assert!(entities[0].1.is_some());
    assert_eq!(entities[1], ("e2", Some("v2")));
    assert_eq!(entities[2], ("e3", Some("v3")));
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}
#[test]
fn solve_list_with_pinned_entities() {
    let p = ListProblem::new(
        vec!["v1", "v2", "v3", "v4", "v5"],
        vec![
            ListEntity::new(vec!["v2", "v1"])
                .with_value_range(vec!["v1", "v2", "v3"])
                .with_pinned(true),
            ListEntity::new(vec!["v5", "v4"])
                .with_value_range(vec!["v1", "v4", "v5"])
                .with_pinned_prefix(2),
            ListEntity::new(vec![]).with_value_range(vec!["v3", "v4", "v5"]),
        ],
    )
    .unwrap();
    let r = lists(&p, true);
    let range = |e: usize| p.entities()[e].value_range().unwrap();
    assert_eq!(
        list_entities(&r.assignment),
        [
            ("e1", vec![range(0)[1], range(0)[0]]),
            ("e2", vec![range(1)[2], range(1)[1]]),
            ("e3", vec![range(2)[0]])
        ]
    );
}
#[test]
fn solve_basic_variable_with_empty_entity_list() {
    let _available_values = ["v1", "v2", "v3"];
    let p = FiniteProblem::new(vec![]).unwrap();
    let r = basic(&p);
    assert!(r.assignment.values().is_empty());
}
#[test]
fn solve_list_variable_with_empty_entity_list() {
    let p = ListProblem::new(vec!["v1", "v2", "v3"], vec![]).unwrap();
    let r = lists(&p, false);
    assert!(r.assignment.lists().is_empty());
}

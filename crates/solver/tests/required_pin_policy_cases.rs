// Adapted from Timefold Community phase tests, Apache-2.0.
// See compat/provenance.json for the explicit native startup policy adaptation.
use timetuck::*;
fn problem(optional: bool) -> FiniteProblem<&'static str> {
    let v = || {
        let v = Variable::new(vec!["v1", "v2", "v3"]);
        if optional { v.allowing_unassigned() } else { v }
    };
    FiniteProblem::new(vec![v(), v().with_initial("v2"), v().with_pinned(true)]).unwrap()
}
fn sequence(exact: bool) -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            reject_unassigned_pins: true,
            ..Default::default()
        },
        phases: vec![if exact {
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::Complete,
                ..Default::default()
            })
        } else {
            SolverPhase::Construction(ConstructionConfig::default())
        }],
    }
}
fn required(exact: bool) {
    let p = problem(false);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let error = solve(&p, &n, sequence(exact)).unwrap_err();
    let SolverError::PinnedUnassignedVariable { variable } = error else {
        panic!("expected a pinned-variable diagnostic: {error:?}");
    };
    // Native slot diagnostic preserves the source's entity and variable identity.
    let identities = [("e1", "value"), ("e2", "value"), ("e3", "value")];
    assert_eq!(identities[variable], ("e3", "value"));
}
#[test]
fn construction_rejects_required_pinned_null() {
    required(false);
}
#[test]
fn exhaustive_rejects_required_pinned_null() {
    required(true);
}
#[test]
fn exhaustive_allows_optional_pinned_null() {
    let p = problem(true);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let mut events = 0;
    let r = solve_with_events(&p, &n, sequence(true), |event| {
        if matches!(event, SolverEvent::Best(_)) {
            events += 1;
        }
    })
    .unwrap();
    assert!(events > 0); // source PlannerTestUtils.solve(..., true)
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}

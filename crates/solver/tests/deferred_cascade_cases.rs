// Adapted from pinned Community BavetRegressionTest.clearEvents and its
// TestdataListMultipleShadowVariable fixtures. Apache-2.0;
// see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn clear_events() {
    // generateSolution(2, 1) assigns only the first value to the sole entity.
    let problem = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0])]).unwrap();
    let mut cache =
        ListCascadeCache::<_, i32>::new_deferred(&problem, &problem.initial_assignment()).unwrap();
    assert_eq!(cache.pending_updates().copied().collect::<Vec<_>>(), [0, 1]);
    assert_eq!(cache.clear_pending_updates(), 2);
    // The source getter returns 2 when its cascade field has never been set.
    assert!(
        problem
            .values()
            .iter()
            .all(|v| cache.shadows()[v].unwrap_or(2) == 2)
    );
    // Extra native evidence: clearing actually cancels scheduled calculations.
    assert!(
        cache
            .flush_pending(|_| panic!("cancelled callback"))
            .unwrap()
            .is_empty()
    );
    assert!(
        problem
            .values()
            .iter()
            .all(|v| cache.shadows()[v].is_none())
    );
}

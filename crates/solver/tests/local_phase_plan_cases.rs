// Adapted pinned Community LocalSearchPhaseConfigTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroU64;
use timetuck::*;
#[test]
fn with_method_calls_properly_chain() {
    let plan = LocalSearchPhasePlan::<HardSoftScore>::default()
        .with_tabu_search(1, true)
        .with_feasible_stop(true)
        .with_accepted_candidate_limit(NonZeroU64::new(5));
    assert!(matches!(
        plan.config().acceptance,
        Acceptance::VariableTabu { .. }
    ));
    assert!(plan.config().stop_when_feasible);
    assert_eq!(plan.config().accepted_candidate_limit.unwrap().get(), 5);
}

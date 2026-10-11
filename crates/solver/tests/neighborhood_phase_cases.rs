// Adapted from pinned Community DefaultSolverTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
#[path = "fixtures/neighborhood_request_fixture.rs"]
mod fixture;
use timetuck::*;
#[test]
fn solve_with_neighborhoods_not_enabled() {
    let domain = basic::Domain::new(3, 2, basic::Objective::FirstPresence);
    assert_eq!(
        domain.model.initial_assignment().values(),
        &[Some(0), Some(1)]
    );
    let mut provider = fixture::TestingNeighborhoodProvider;
    let request = NeighborhoodPhase::local_search(LocalSearchConfig {
        best_score_limit: Some(SimpleScore::ZERO),
        ..Default::default()
    });
    let result: Result<
        PreparedNeighborhoodPhase<'_, basic::Model, Assignment<i64>, Move<i64>, SimpleScore>,
        _,
    > = request.prepare(NeighborhoodApi::default(), Some(&mut provider));
    let error = result.err().unwrap();
    assert_eq!(error, NeighborhoodApiError::FeatureDisabled);
    assert!(error.to_string().contains("NEIGHBORHOODS"));
}
#[test]
fn neighborhoods_rejects_variable_neighborhood_descent() {
    let domain = basic::Domain::new(3, 2, basic::Objective::FirstPresence);
    assert_eq!(
        domain.model.initial_assignment().values(),
        &[Some(0), Some(1)]
    );
    let union = NeighborhoodUnion {
        children: &[(Neighborhood::Change, 1), (Neighborhood::Swap, 1)],
        order: UnionOrder::Sequential,
    };
    let request = NeighborhoodPhase::variable_descent(
        &union,
        LocalSearchConfig {
            best_score_limit: Some(SimpleScore::ZERO),
            ..Default::default()
        },
    )
    .unwrap();
    let result: Result<
        PreparedNeighborhoodPhase<'_, basic::Model, Assignment<i64>, Move<i64>, SimpleScore>,
        _,
    > = request.prepare(NeighborhoodApi { enabled: true }, None);
    let error = result.err().unwrap();
    assert_eq!(error, NeighborhoodApiError::DescentUnsupported);
    assert!(
        error
            .to_string()
            .contains("does not support the Neighborhoods API")
    );
}

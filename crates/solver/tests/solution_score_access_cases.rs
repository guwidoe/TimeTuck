// Complete pinned Community ScoreDescriptorTest adaptations, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/score_access_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn score_definition() {
    let accessor = accessor();
    let definition: TypedScoreDefinition<SimpleScore> = accessor.score_definition();
    assert_eq!(definition.score_type(), PlanningType::of::<SimpleScore>());
    assert_eq!(definition.zero_score(), SimpleScore::new(0));
    assert_eq!(definition.level_count(), 1);
}
#[test]
fn score_access() {
    let accessor = accessor();
    let mut solution = Solution::default();
    assert!(accessor.read(&solution).is_none());
    let score = Arc::new(SimpleScore::new(-2));
    accessor.write(&mut solution, Some(score.clone()));
    assert!(Arc::ptr_eq(accessor.read(&solution).unwrap(), &score));
}

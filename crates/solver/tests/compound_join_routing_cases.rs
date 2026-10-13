// Adapted from Community ComparisonIndexerTest.joinIndexEligibilityRouting,
// Apache-2.0. See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn join_index_eligibility_routing() {
    // Native conjunctions carry the exact TEST age/gender projections. The same
    // query selects partitioned versus standalone indexes when applied to a graph.
    let age = JoinProjection::new(|person: &(u8, i32)| person.1);
    let gender = JoinProjection::new(|person: &(u8, i32)| person.0);
    let less = JoinCondition::new(JoinComparison::LessThan, &age, &age);
    let greater = JoinCondition::new(JoinComparison::GreaterThan, &age, &age);
    let equal = JoinCondition::new(JoinComparison::Equal, &gender, &gender);
    assert!(!JoinConditions::new([less.clone()]).uses_equality_partitions());
    assert!(!JoinConditions::new([less.clone(), greater]).uses_equality_partitions());
    assert!(JoinConditions::new([equal.clone()]).uses_equality_partitions());
    assert!(JoinConditions::new([equal, less]).uses_equality_partitions());
}

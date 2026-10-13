// Adapted from pinned Community SubListTest and PillarDemandTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn sublist_equals() {
    let expected = SubListSpan::new("A", 3, 9).unwrap();
    for (entity, start, length) in [("B", 3, 9), ("A", 9, 3), ("A", 9, 9), ("A", 3, 3)] {
        assert_ne!(SubListSpan::new(entity, start, length).unwrap(), expected);
    }
    assert_eq!(SubListSpan::new("A", 3, 9).unwrap(), expected);
}
#[test]
fn sublist_to_string() {
    assert_eq!(SubListSpan::new("A", 2, 5).unwrap().to_string(), "A[2..7]");
}
struct Solution;
#[test]
fn pillar_demand_equality() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<usize>(vec![
            PlanningVariableDefinition::basic::<usize>("value", false),
        ])])
        .unwrap();
    let variables = schema
        .entity::<usize>()
        .unwrap()
        .genuine_variables()
        .collect::<Vec<_>>();
    let policy = PillarSelectionPolicy::whole();
    let source = PillarEntitySource::new(|| vec![0usize, 1]);
    let filter = PillarEntityFilter::new(|_: &usize| true);
    let demand = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        variables.clone(),
        policy.clone(),
    )
    .unwrap();
    assert_eq!(demand, demand.clone());
    let same = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        variables.clone(),
        policy.clone(),
    )
    .unwrap();
    assert_eq!(same, demand);
    let copied_list = PillarSelectionDemand::new(
        source.clone(),
        filter.clone(),
        variables.to_vec(),
        policy.clone(),
    )
    .unwrap();
    assert_eq!(copied_list, demand);
    let repeated_wrapper =
        PillarSelectionDemand::new(source, filter, variables.to_vec(), policy).unwrap();
    assert_eq!(repeated_wrapper, demand);
}

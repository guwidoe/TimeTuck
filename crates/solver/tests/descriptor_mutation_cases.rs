// Adapted pinned Community MutationCounterTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/descriptor_mutation_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn count_mutations_on_extended_entities() {
    let c = composition();
    let layout = layout(3, 7, 17);
    let p = problem(&layout);
    let tracker = tracker(&c, &p, &layout);
    let a = tracker.snapshot(&p.initial_assignment());
    let b = tracker.snapshot(&Assignment::new(vec![Some(2); p.variables().len()]));
    let report = DescriptorMutationViews::all(&c)
        .unwrap()
        .compare(&a, &b)
        .unwrap();
    let base_descriptor_count = 3 + 7 + 17 + 2;
    let child_descriptor_count = 7 * 2 + 17 * 2 + 2;
    assert_eq!(
        report.occurrence_count(),
        base_descriptor_count + child_descriptor_count
    );
    assert_eq!(
        report.descriptor_changes,
        vec![
            (PlanningType::of::<Base>(), base_descriptor_count),
            (PlanningType::of::<Child>(), child_descriptor_count)
        ]
    );
    assert_eq!(report.unique_changes, a.mutation_count(&b).unwrap());
    assert_eq!(report.unique_changes, 54);
}

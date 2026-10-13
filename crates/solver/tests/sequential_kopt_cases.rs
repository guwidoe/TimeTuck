// Complete pinned Community odd-sequential iterator TEST assertions, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/sequential_kopt_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn verify(selector: &ListKOptSequentialSelector, owners: &[usize]) {
    let f = fixture(owners);
    let a = f.problem.initial_assignment();
    let mut origin = Some(f.origin);
    let mut values = f.values.into_iter();
    let mut directions = 0;
    let proposal = selector
        .select(
            &f.problem,
            &a,
            (owners.len() - 2) as u64,
            || origin.take(),
            || values.next(),
            || {
                directions += 1;
                true
            },
        )
        .unwrap();
    assert_eq!(proposal.k(), owners.len());
    assert_eq!(proposal.removed_values(), f.expected);
    assert_eq!(
        proposal.endpoint_partners(),
        expected_partners(owners.len())
    );
    assert_eq!(directions, owners.len());
    assert!(values.next().is_none());
    assert_eq!(a, f.problem.initial_assignment());
}
#[test]
fn test_sequential_k_opt_on_same_entity() {
    let selector =
        ListKOptSequentialSelector::new(ListKOptConfig::new(2, 5).unwrap(), vec![1, 1, 1, 1])
            .unwrap();
    verify(&selector, &[0, 0, 0]);
    verify(&selector, &[0, 0, 0, 0, 0]);
}
#[test]
fn test_sequential_k_opt_on_different_entities() {
    let selector =
        ListKOptSequentialSelector::new(ListKOptConfig::new(2, 5).unwrap(), vec![1, 1, 1, 1])
            .unwrap();
    verify(&selector, &[0, 1, 0, 1, 0]);
}

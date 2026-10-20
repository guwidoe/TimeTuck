// Pinned Community iterator TEST adaptations (native descriptors), Apache-2.0.
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

#[path = "fixtures/nonsequential_kopt_fixture.rs"]
mod nonsequential_fixture;

fn verify_nonsequential(owners: &[usize; 5]) {
    use nonsequential_fixture::{JAVA_PARTNERS, native_partners, nonsequential_fixture};
    let f = nonsequential_fixture(owners);
    let before = f.problem.initial_assignment();
    let mut origin = Some(f.origin);
    let mut values = f.values.into_iter();
    let mut directions = 0;
    let mut repair_draws = 0;
    let proposal =
        ListKOptSubtourRepairSelector::new(ListKOptConfig::new(2, 6).unwrap(), vec![1, 1, 1, 1])
            .unwrap()
            .select(
                &f.problem,
                &before,
                2,
                || origin.take(),
                || values.next(),
                || {
                    directions += 1;
                    true
                },
                |components, removed| {
                    repair_draws += 1;
                    assert_eq!(components.len(), 2);
                    assert_eq!(removed.len(), 4);
                    Some((f.repair_cut, true))
                },
            )
            .unwrap();
    // All source descriptor assertions in the native representation: chosen k,
    // every removed endpoint in draw/repair order, and every active partner.
    // Java sentinel and spare buffer slots are explicitly not a native ABI.
    assert_eq!(proposal.k(), f.expected_removed.len() / 2);
    assert_eq!(proposal.removed_values(), f.expected_removed);
    assert_eq!(
        proposal.active_endpoint_partners(),
        native_partners(&JAVA_PARTNERS, proposal.k()).unwrap()
    );
    assert_eq!(directions, 4);
    assert_eq!(repair_draws, 1);
    assert!(origin.is_none());
    assert!(values.next().is_none());
    assert_eq!(proposal.initial_component_count(), 2);
    assert_eq!(proposal.physical_plan().physical_cycle_count(), 1);
    assert_eq!(before, f.problem.initial_assignment());

    // Execute and undo the computed plan; expected endpoint arrays do not feed it.
    let lengths: Vec<_> = before.lists().iter().map(Vec::len).collect();
    let movement = proposal
        .physical_plan()
        .movement(
            &f.problem,
            &before,
            ListPosition {
                entity: owners[0],
                index: 0,
            },
            false,
            &lengths,
        )
        .unwrap();
    let mut current = before.clone();
    {
        let _undo = f.problem.apply(&mut current, &movement).unwrap();
    }
    assert_eq!(current, before);
    f.problem.apply(&mut current, &movement).unwrap().commit();
    f.problem.validate(&current).unwrap();
    assert_ne!(current, before);
}

#[test]
fn test_nonsequential_k_opt_on_same_entity() {
    verify_nonsequential(&[0, 0, 0, 0, 0]);
}

#[test]
fn test_nonsequential_k_opt_on_different_entity() {
    verify_nonsequential(&[0, 1, 0, 1, 0]);
}

#[test]
fn nonsequential_descriptor_translation_preserves_active_connections() {
    use nonsequential_fixture::{JAVA_PARTNERS, native_partners};
    let expected = native_partners(&JAVA_PARTNERS, 6).unwrap();
    let mut changed_spare = JAVA_PARTNERS;
    changed_spare[13..].copy_from_slice(&[14, 13, 16, 15]);
    assert_eq!(native_partners(&changed_spare, 6).unwrap(), expected);
    assert_eq!(native_partners(&JAVA_PARTNERS[..13], 6).unwrap(), expected);
    let mut changed_active = JAVA_PARTNERS;
    changed_active[1] = 2;
    changed_active[2] = 1;
    changed_active[3] = 8;
    changed_active[8] = 3;
    assert_ne!(native_partners(&changed_active, 6).unwrap(), expected);
    for bad in [0, 1, 13, usize::MAX] {
        let mut invalid = JAVA_PARTNERS;
        invalid[1] = bad;
        assert!(native_partners(&invalid, 6).is_err());
    }
    assert!(native_partners(&JAVA_PARTNERS[..12], 6).is_err());
}

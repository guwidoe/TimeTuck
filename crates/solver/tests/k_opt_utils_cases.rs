// Adapted from pinned Community KOptUtilsTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use num_bigint::BigUint;
use timetuck::*;

// One-based native positions preserve v1..v12 identity directly. No Java
// endpoint permutation or mocked variable state is needed by this descriptor.
fn descriptor(n: usize, added: &[(usize, usize)]) -> ListKOptTopology {
    let removed: Vec<_> = (1..=n).step_by(2).map(|i| (i, i + 1)).collect();
    ListKOptTopology::new(1..n + 1, &removed, added).unwrap()
}
fn one_cycle() -> ListKOptTopology {
    descriptor(8, &[(1, 4), (2, 7), (5, 3), (6, 8)])
}
#[test]
fn get_removed_edges_list() {
    assert_eq!(
        one_cycle().removed_edges(),
        &[(1, 2), (3, 4), (5, 6), (7, 8)]
    );
}
#[test]
fn get_added_edges_list() {
    assert_eq!(one_cycle().added_edges(), &[(1, 4), (2, 7), (5, 3), (6, 8)]);
}
#[test]
fn cycles_for_permutation_one_cycle() {
    let t = one_cycle();
    assert_eq!(t.cycle_count(), 1);
    assert_eq!(
        [&[0][..], t.cycle_ids()].concat(),
        [0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
#[test]
fn cycles_for_permutation_two_cycles() {
    let t = descriptor(8, &[(1, 4), (3, 6), (5, 8), (7, 2)]);
    assert_eq!(t.cycle_count(), 2);
    assert_eq!(
        [&[0][..], t.cycle_ids()].concat(),
        [0, 0, 1, 1, 0, 0, 1, 1, 0]
    );
}
#[test]
fn cycles_for_permutation_three_cycles() {
    let t = descriptor(12, &[(1, 4), (5, 12), (3, 6), (7, 2), (8, 10), (11, 9)]);
    assert_eq!(t.cycle_count(), 3);
    assert_eq!(
        [&[0][..], t.cycle_ids()].concat(),
        [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 2, 2, 0]
    );
}
fn flip(mut a: [i32; 8], start: usize, end: usize) {
    let original = a;
    reverse_cyclic_span(&mut a, start, end).unwrap();
    assert_eq!(a, [1, 2, 3, 4, 5, 6, 7, 8]);
    reverse_cyclic_span(&mut a, start, end).unwrap();
    assert_eq!(a, original);
}
#[test]
fn flip_subarray() {
    flip([1, 2, 5, 4, 3, 6, 7, 8], 2, 5);
}
#[test]
fn flip_subarray_second_ends_before_first() {
    flip([8, 7, 3, 4, 5, 6, 2, 1], 6, 2);
}
#[test]
fn flip_subarray_second_ends_before_first_unbalanced() {
    flip([6, 5, 3, 4, 2, 1, 8, 7], 4, 2);
}
#[test]
fn flip_subarray_first_ends_before_second_unbalanced() {
    flip([2, 1, 8, 7, 5, 6, 4, 3], 6, 4);
}
#[test]
fn pure_k_opt_move_types() {
    for (k, expected) in (2..=8).zip([1u64, 4, 25, 208, 2121, 25828, 365457]) {
        assert_eq!(pure_k_opt_topology_count(k), BigUint::from(expected));
    }
}

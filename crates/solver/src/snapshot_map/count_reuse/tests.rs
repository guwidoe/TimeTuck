//! Original allocation-identity, AVL, snapshot and journal properties.
use super::*;
use std::collections::HashSet;

fn inspect(root: &Link<i64, usize>, rows: &mut Vec<(i64, usize)>) -> (usize, usize) {
    let Some(node) = root else { return (0, 0) };
    let (left_height, left_size) = inspect(&node.left, rows);
    if let Some(&(key, _)) = rows.last() {
        assert!(key < *node.key);
    }
    rows.push((*node.key, *node.value));
    let (right_height, right_size) = inspect(&node.right, rows);
    let height = 1 + left_height.max(right_height);
    let size = 1 + left_size + right_size;
    assert!(left_height.abs_diff(right_height) <= 1);
    assert_eq!(node.height, height);
    assert_eq!(node.size, size);
    (height, size)
}
fn rows(map: &SnapshotMap<i64, usize>) -> Vec<(i64, usize)> {
    let mut rows = Vec::new();
    inspect(&map.root, &mut rows);
    assert_eq!(rows.len(), map.len());
    rows
}
fn owners(root: &Link<i64, usize>, out: &mut [HashSet<usize>; 3]) {
    if let Some(node) = root {
        out[0].insert(Arc::as_ptr(node) as usize);
        out[1].insert(Arc::as_ptr(&node.key) as usize);
        out[2].insert(Arc::as_ptr(&node.value) as usize);
        owners(&node.left, out);
        owners(&node.right, out);
    }
}

#[test]
fn unique_renames_retain_every_node_key_and_value_allocation_and_avl_invariant() {
    for len in [1, 2, 3, 7, 31, 127] {
        let mut map =
            SnapshotMap::from_sorted((0..len).map(|key| (key, 1)).collect::<Vec<_>>().into_iter());
        let mut original_owners = std::array::from_fn(|_| HashSet::new());
        owners(&map.root, &mut original_owners);
        for old in (0..len).map(|index| (index * 37) % len) {
            assert!(map.retarget_singleton(old, old + 1000));
            let expected = (0..len)
                .filter(|&key| key != old)
                .map(|key| (key, 1))
                .chain([(old + 1000, 1)])
                .collect::<Vec<_>>();
            assert_eq!(rows(&map), expected);
            let mut current_owners = std::array::from_fn(|_| HashSet::new());
            owners(&map.root, &mut current_owners);
            assert_eq!(current_owners, original_owners);
            assert!(map.retarget_singleton(old + 1000, old));
            assert_eq!(rows(&map), (0..len).map(|key| (key, 1)).collect::<Vec<_>>());
            let mut restored_owners = std::array::from_fn(|_| HashSet::new());
            owners(&map.root, &mut restored_owners);
            assert_eq!(restored_owners, original_owners);
        }
    }
}

#[test]
fn shared_key_owners_and_shared_tree_snapshots_survive_primitive_retargeting() {
    let mut map =
        SnapshotMap::from_sorted((0..31).map(|key| (key, 1)).collect::<Vec<_>>().into_iter());
    // Key owners can be shared independently while all tree nodes stay unique.
    let key_holders = map.shared_entries();
    assert!(map.retarget_singleton(15, 1000));
    assert_eq!(
        key_holders
            .iter()
            .map(|(key, value)| (**key, **value))
            .collect::<Vec<_>>(),
        (0..31).map(|key| (key, 1)).collect::<Vec<_>>()
    );
    let saved = map.clone();
    let expected = rows(&saved);
    assert!(map.retarget_singleton(1000, -1));
    assert_eq!(rows(&saved), expected);
    assert_eq!(
        rows(&map),
        std::iter::once((-1, 1))
            .chain((0..31).filter(|&key| key != 15).map(|key| (key, 1)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn retargeting_preserves_nested_value_journals_snapshots_and_structural_rollback() {
    for shared in [false, true] {
        for commit_inner in [false, true] {
            let mut map = SnapshotMap::from_sorted(
                (0..31).map(|key| (key, 1)).collect::<Vec<_>>().into_iter(),
            );
            let original = rows(&map);
            let original_snapshot = shared.then(|| map.clone());
            let outer = map.begin_journal();
            map.insert(0, 2);
            let middle = rows(&map);
            let inner = map.begin_journal();
            assert!(map.retarget_singleton(15, 1000));
            let held = map.clone();
            let expected = rows(&held);
            assert_eq!(rows(&map.journal_snapshot(outer)), original);
            assert_eq!(rows(&map.journal_snapshot(inner)), middle);
            assert!(map.retarget_singleton(1000, -1));
            if commit_inner {
                map.commit_journal(inner);
            } else {
                map.rollback_journal(inner);
                assert_eq!(rows(&map), middle);
            }
            map.rollback_journal(outer);
            assert_eq!(rows(&map), original);
            assert_eq!(rows(&held), expected);
            if let Some(saved) = original_snapshot {
                assert_eq!(rows(&saved), original);
            }
        }
    }
}

#[test]
fn ineligible_renames_leave_the_exact_original_root_unchanged() {
    let mut map = SnapshotMap::from_sorted(
        (0..16)
            .map(|key| (key, if key == 8 { 2 } else { 1 }))
            .collect::<Vec<_>>()
            .into_iter(),
    );
    let saved = map.clone();
    for (old, new) in [(8, 99), (7, 8), (7, 7), (99, 100)] {
        assert!(!map.retarget_singleton(old, new));
        assert!(map.shares_root(&saved));
        assert_eq!(rows(&map), rows(&saved));
    }
    let mut empty = SnapshotMap::<i64, usize>::new();
    assert!(!empty.retarget_singleton(0, 1));
    assert!(empty.is_empty());
}

#[test]
fn two_child_detach_keeps_the_successor_representative_and_duplicate_count() {
    // Sorted construction makes 3 the root and 4 its in-order successor.
    let mut map = SnapshotMap::from_sorted(
        (0..7)
            .map(|key| (key, if key == 4 { 2 } else { 1 }))
            .collect::<Vec<_>>()
            .into_iter(),
    );
    let successor = map.get_key_value(&4).unwrap().0 as *const i64;
    let mut original_owners = std::array::from_fn(|_| HashSet::new());
    owners(&map.root, &mut original_owners);
    assert!(map.retarget_singleton(3, 99));
    assert_eq!(
        rows(&map),
        vec![(0, 1), (1, 1), (2, 1), (4, 2), (5, 1), (6, 1), (99, 1)]
    );
    assert_eq!(map.get_key_value(&4).unwrap().0 as *const i64, successor);
    let mut current_owners = std::array::from_fn(|_| HashSet::new());
    owners(&map.root, &mut current_owners);
    assert_eq!(current_owners, original_owners);
}

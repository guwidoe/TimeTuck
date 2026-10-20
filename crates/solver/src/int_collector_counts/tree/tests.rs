//! Original frequency, ownership and snapshot properties; no upstream credit.
use super::*;
use crate::int_collector_counts::IntCollectorCounts;
use crate::{IntCollector, ReversibleCollector, SumCollector};
use std::collections::{BTreeMap, HashSet};

fn inspect(root: &Link, rows: &mut Vec<(i64, usize)>) -> (usize, usize) {
    let Some(n) = root else { return (0, 0) };
    let (lh, ls) = inspect(&n.left, rows);
    if let Some(&(previous, _)) = rows.last() {
        assert!(previous < n.key);
    }
    assert!(n.count > 0);
    rows.push((n.key, n.count));
    let (rh, rs) = inspect(&n.right, rows);
    assert!(lh.abs_diff(rh) <= 1);
    assert_eq!(n.height, 1 + lh.max(rh));
    assert_eq!(n.size, 1 + ls + rs);
    (n.height, n.size)
}
fn rows(tree: &CountsTree) -> Vec<(i64, usize)> {
    let mut rows = Vec::new();
    inspect(&tree.root, &mut rows);
    assert_eq!(tree.iter().collect::<Vec<_>>(), rows);
    assert_eq!(tree.len(), rows.len());
    assert_eq!(tree.min(), rows.first().map(|r| r.0));
    assert_eq!(tree.max(), rows.last().map(|r| r.0));
    rows
}
fn owners(root: &Link, nodes: &mut HashSet<usize>) {
    if let Some(n) = root {
        nodes.insert(Arc::as_ptr(n) as usize);
        owners(&n.left, nodes);
        owners(&n.right, nodes);
    }
}

#[test]
fn generated_edits_match_standard_frequencies_through_rotations_and_snapshots() {
    let mut tree = CountsTree::new();
    let mut expected = BTreeMap::<i64, usize>::new();
    let mut saved = Vec::new();
    let mut seed = 281_u64;
    for step in 0..20_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = (seed >> 32) as i64 % 257 - 128;
        if seed & 3 == 0 {
            let present = expected.contains_key(&key);
            assert_eq!(tree.remove(key), present);
            if present {
                let count = expected.get_mut(&key).unwrap();
                *count -= 1;
                if *count == 0 {
                    expected.remove(&key);
                }
            }
        } else {
            tree.increment(key);
            *expected.entry(key).or_default() += 1;
        }
        assert_eq!(
            rows(&tree),
            expected.iter().map(|(&k, &n)| (k, n)).collect::<Vec<_>>()
        );
        if step % 991 == 0 {
            saved.push((tree.clone(), expected.clone()));
        }
    }
    for (tree, expected) in saved {
        assert_eq!(rows(&tree), expected.into_iter().collect::<Vec<_>>());
    }
    for (key, count) in expected {
        for _ in 0..count {
            assert!(tree.remove(key));
        }
        rows(&tree);
    }
    assert!(tree.root.is_none());
}

#[test]
fn unique_renames_reuse_all_nodes_and_preserve_duplicate_successors() {
    for len in [1, 2, 3, 7, 31, 127] {
        let mut tree = CountsTree::new();
        for key in 0..len {
            tree.increment(key);
        }
        // Give some in-order successors duplicate counts.
        for key in (0..len).filter(|k| k % 3 == 1) {
            tree.increment(key);
        }
        let before = rows(&tree);
        let mut initial = HashSet::new();
        owners(&tree.root, &mut initial);
        for old in (0..len).filter(|k| k % 3 != 1) {
            assert!(tree.retarget_singleton(old, old + 1000));
            let mut expected = before
                .iter()
                .copied()
                .filter(|r| r.0 != old)
                .collect::<Vec<_>>();
            expected.push((old + 1000, 1));
            assert_eq!(rows(&tree), expected);
            let mut current = HashSet::new();
            owners(&tree.root, &mut current);
            assert_eq!(current, initial);
            assert!(tree.retarget_singleton(old + 1000, old));
            assert_eq!(rows(&tree), before);
        }
    }
}

#[test]
fn snapshots_copy_only_changed_paths_and_keep_original_extreme_counts() {
    let mut tree = CountsTree::new();
    for key in 0..8192 {
        tree.increment(key);
    }
    tree.increment(i64::MIN);
    tree.increment(i64::MAX);
    let saved = tree.clone();
    let expected = rows(&saved);
    let mut original = HashSet::new();
    owners(&saved.root, &mut original);
    for key in [i64::MIN, 0, 4095, 8191, i64::MAX] {
        let mut candidate = saved.clone();
        assert!(candidate.retarget_singleton(key, -1));
        let mut changed = HashSet::new();
        owners(&candidate.root, &mut changed);
        assert!(changed.difference(&original).count() <= 8 * height(&saved.root) + 2);
        candidate.increment(-1);
        candidate.remove(-1);
        assert_eq!(rows(&saved), expected);
        assert_eq!(candidate.get(key), None);
        assert_eq!(candidate.get(-1), Some(1));
    }
}

#[test]
fn missing_edits_and_ineligible_renames_preserve_the_exact_root() {
    let mut tree = CountsTree::new();
    for key in 0..31 {
        tree.increment(key);
    }
    tree.increment(8);
    let saved = tree.clone();
    for (old, new) in [(8, 99), (7, 8), (7, 7), (99, 100)] {
        assert!(!tree.retarget_singleton(old, new));
        assert!(Arc::ptr_eq(
            tree.root.as_ref().unwrap(),
            saved.root.as_ref().unwrap()
        ));
    }
    assert!(!tree.remove(99));
    assert!(Arc::ptr_eq(
        tree.root.as_ref().unwrap(),
        saved.root.as_ref().unwrap()
    ));
    assert_eq!(rows(&tree), rows(&saved));
    let mut empty = CountsTree::new();
    assert!(!empty.retarget_singleton(0, 1));
    assert!(!empty.remove(0));
}

#[test]
fn promotion_copies_large_counts_once_and_demotion_keeps_multiplicities() {
    let mut counts = IntCollectorCounts::Inline {
        entries: [(1, usize::MAX - 2), (2, 1)],
        len: 2,
    };
    let saved = counts.clone();
    counts.increment(3);
    let IntCollectorCounts::Tree(tree) = &counts else {
        panic!("expected promotion")
    };
    assert_eq!(rows(tree), [(1, usize::MAX - 2), (2, 1), (3, 1)]);
    assert!(counts.remove(3));
    assert_eq!(counts, saved);
    assert!(matches!(counts, IntCollectorCounts::Tree(_)));
    assert!(counts.remove(2));
    let IntCollectorCounts::Inline { entries, len } = counts else {
        panic!("expected demotion")
    };
    assert_eq!(len, 1);
    assert_eq!(entries[0], (1, usize::MAX - 2));
}

#[test]
fn equality_is_independent_of_history_shape_and_shared_roots() {
    let mut left = CountsTree::new();
    let mut right = CountsTree::new();
    for key in 0..64 {
        left.increment(key);
    }
    for key in (0..64).rev() {
        right.increment(key);
    }
    assert_eq!(left, right);
    let saved = left.clone();
    left.increment(1);
    assert_ne!(left, saved);
    assert_ne!(left, right);
    right.increment(1);
    assert_eq!(left, right);
}

#[test]
fn real_integer_collectors_preserve_captures_and_checked_failure_across_clones() {
    let mut sums = SumCollector::default();
    let mut integers = IntCollector::default();
    let values = (0..1024_i64).map(|k| k - 512).collect::<Vec<_>>();
    for &key in &values {
        sums.try_insert(key).unwrap();
        integers.insert(key);
    }
    let sum_saved = sums.clone();
    let int_saved = integers.clone();
    let initial = integers.summary();
    for &old in &[i64::MIN, -512, 0, 511, i64::MAX] {
        let mut s = sum_saved.clone();
        let mut c = int_saved.clone();
        let present = values.contains(&old);
        assert_eq!(s.try_replace(old, 4096), Ok(present));
        assert_eq!(ReversibleCollector::replace(&mut c, &old, &4096), present);
        if present {
            assert_eq!(s.finish(), -512 + 4096 - old);
            assert_eq!(c.summary().sum, s.finish());
            assert_eq!(c.summary().max, Some(4096));
            assert!(s.try_replace(4096, old).unwrap());
            assert!(ReversibleCollector::replace(&mut c, &4096, &old));
            assert_eq!(s, sum_saved);
            assert_eq!(c, int_saved);
        }
    }
    assert_eq!(integers.summary(), initial);
    assert!(sums.try_replace(-512, i64::MAX).is_err());
    assert_eq!(sums, sum_saved);
    assert_eq!(sums.try_replace(i64::MIN, i64::MAX), Ok(false));
    assert_eq!(sums, sum_saved);
}

#[test]
fn shared_frequency_roots_remain_independent_across_threads() {
    let mut initial = IntCollector::default();
    for k in -128..128 {
        initial.insert(k);
    }
    let original = initial.summary();
    let handles = (0..4)
        .map(|i| {
            let mut branch = initial.clone();
            std::thread::spawn(move || {
                assert!(ReversibleCollector::replace(&mut branch, &i, &(i + 1000)));
                branch.summary()
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        let changed = handle.join().unwrap();
        assert_eq!(changed.count, original.count);
        assert_eq!(changed.sum, original.sum + 1000);
    }
    assert_eq!(initial.summary(), original);
}

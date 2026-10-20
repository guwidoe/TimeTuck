//! Original mixed frequency histories; checked/wrapping arithmetic rules stay existing.
use std::collections::BTreeMap;
use timetuck::{IntCollector, ReversibleCollector, SumCollector};

fn reference(counts: &BTreeMap<i64, usize>) -> (SumCollector, IntCollector) {
    let mut checked = SumCollector::default();
    let mut wrapping = IntCollector::default();
    for (&value, &count) in counts {
        for _ in 0..count {
            checked.try_insert(value).unwrap();
            wrapping.insert(value);
        }
    }
    (checked, wrapping)
}

#[test]
fn mixed_replacements_preserve_all_frequencies_clones_and_inline_tree_boundaries() {
    let mut counts = BTreeMap::new();
    for value in -16..=16 {
        counts.insert(value, if value % 3 == 0 { 2 } else { 1 });
    }
    let (mut checked, mut wrapping) = reference(&counts);
    let initial = (checked.clone(), wrapping.clone());
    let mut seed = 279u64;
    for step in 0..512 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let old = (seed >> 32) as i64 % 49 - 24;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let new = if step % 7 == 0 {
            old
        } else {
            (seed >> 32) as i64 % 49 - 24
        };
        let before = (checked.clone(), wrapping.clone());
        let before_expected = reference(&counts);
        let found = counts.get(&old).copied().is_some_and(|count| count > 0);
        if found {
            let count = counts.get_mut(&old).unwrap();
            *count -= 1;
            if *count == 0 {
                counts.remove(&old);
            }
            *counts.entry(new).or_default() += 1;
        }
        assert_eq!(checked.try_replace(old, new), Ok(found));
        assert_eq!(
            ReversibleCollector::replace(&mut wrapping, &old, &new),
            found
        );
        assert_eq!((checked.clone(), wrapping.clone()), reference(&counts));
        assert_eq!(before, before_expected);
    }
    // Drain every occurrence through the public APIs, forcing demotion to inline.
    for (&value, &count) in &counts {
        for _ in 0..count {
            assert_eq!(checked.try_remove(value), Ok(true));
            assert!(wrapping.remove(value));
        }
        assert_eq!(checked.try_remove(value), Ok(false));
        assert!(!wrapping.remove(value));
    }
    assert_eq!(
        (checked.clone(), wrapping.clone()),
        reference(&BTreeMap::new())
    );
    assert_eq!(checked.try_replace(0, 1), Ok(false));
    assert!(!ReversibleCollector::replace(&mut wrapping, &0, &1));
    let mut small_counts = BTreeMap::new();
    for value in [0, 0, 1, 2, 3, -1] {
        checked.try_insert(value).unwrap();
        wrapping.insert(value);
        *small_counts.entry(value).or_default() += 1;
    }
    for (old, new) in [(0, 9), (1, 9), (2, -9), (9, 9), (44, 0), (3, 0)] {
        let found = if let Some(count) = small_counts.get_mut(&old) {
            *count -= 1;
            if *count == 0 {
                small_counts.remove(&old);
            }
            *small_counts.entry(new).or_default() += 1;
            true
        } else {
            false
        };
        assert_eq!(checked.try_replace(old, new), Ok(found));
        assert_eq!(
            ReversibleCollector::replace(&mut wrapping, &old, &new),
            found
        );
        assert_eq!(
            (checked.clone(), wrapping.clone()),
            reference(&small_counts)
        );
    }
    let mut original_counts = BTreeMap::new();
    for value in -16..=16 {
        original_counts.insert(value, if value % 3 == 0 { 2 } else { 1 });
    }
    assert_eq!(initial, reference(&original_counts));
}

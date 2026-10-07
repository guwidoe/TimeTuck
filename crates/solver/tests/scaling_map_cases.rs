// Adapted from pinned Community ScalingNavigableMapTest (Apache-2.0).
// Native tuning and RNG replace referenced Java constants/Random; see provenance.
use std::{collections::BTreeMap, num::NonZeroUsize};
use timetuck::*;
fn fill(count: usize) -> ScalingMap<usize, String> {
    let mut map = ScalingMap::new();
    for key in 0..count {
        map.get_or_insert_with(key, || format!("value{key}"));
    }
    map
}
fn treeified() -> ScalingMap<usize, String> {
    let c = ScalingMapConfig::default();
    let mut map = fill(c.array_threshold + 1);
    for _ in 0..c.churn_tolerance {
        map.remove(&0);
        map.get_or_insert_with(0, || "value0".into());
    }
    assert!(!map.is_array_based());
    map
}
fn keys(map: &ScalingMap<usize, String>) -> Vec<usize> {
    map.iter().map(|(k, _)| *k).collect()
}
#[test]
fn get_or_create_get_remove() {
    let mut map = ScalingMap::new();
    assert!(map.is_empty());
    assert_eq!(map.get(&1), None);
    assert_eq!(*map.get_or_insert_with(1, || "one"), "one");
    assert_eq!(
        *map.get_or_insert_with(1, || panic!("should not be used")),
        "one"
    );
    assert_eq!(map.get(&1), Some(&"one"));
    assert_eq!(map.len(), 1);
    assert!(!map.is_empty());
    map.remove(&1);
    assert_eq!(map.get(&1), None);
    assert!(map.is_empty());
}
#[test]
fn remove_is_no_op_for_missing_key() {
    let mut map = ScalingMap::new();
    map.get_or_insert_with(1, || "one");
    map.get_or_insert_with(2, || "two");
    map.remove(&99);
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&1), Some(&"one"));
    assert_eq!(map.get(&2), Some(&"two"));
}
#[test]
fn iterator_honors_reversed_in_tree_mode() {
    let map = treeified();
    assert!(!map.is_array_based());
    assert!(keys(&map).is_sorted());
    let reverse: Vec<_> = map.iter().rev().map(|(k, _)| *k).collect();
    assert!(reverse.windows(2).all(|p| p[0] > p[1]));
}
#[test]
fn treeifies_on_churn_at_threshold_and_stays_treeified() {
    let mut map = treeified();
    let threshold = ScalingMapConfig::default().array_threshold;
    assert!(!map.is_array_based());
    assert_eq!(map.len(), threshold + 1);
    assert_eq!(keys(&map).len(), threshold + 1);
    assert!(keys(&map).is_sorted());
    for key in 1..=threshold {
        map.remove(&key);
    }
    assert!(!map.is_array_based());
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&0).map(String::as_str), Some("value0"));
}
#[test]
fn growth_without_churn_stays_array_based() {
    let n = ScalingMapConfig::default().maximum_array_size;
    let map = fill(n);
    assert!(map.is_array_based());
    assert_eq!(map.len(), n);
    assert!(keys(&map).is_sorted());
    assert_eq!(map.get(&(n / 2)), Some(&format!("value{}", n / 2)));
}
#[test]
fn churn_below_threshold_stays_array_based() {
    let c = ScalingMapConfig::default();
    let n = c.array_threshold / 2;
    let mut map = fill(n);
    for _ in 0..c.churn_tolerance * 4 {
        map.remove(&0);
        map.get_or_insert_with(0, || "churned".into());
        assert!(map.is_array_based());
    }
    assert_eq!(map.len(), n);
}
#[test]
fn churn_at_threshold_treeifies_only_after_tolerance() {
    let c = ScalingMapConfig::default();
    let n = c.array_threshold + 1;
    let mut map = fill(n);
    assert!(map.is_array_based());
    for _ in 0..c.churn_tolerance - 1 {
        map.remove(&0);
        map.get_or_insert_with(0, || "rebuilt0".into());
    }
    assert!(map.is_array_based());
    map.remove(&0);
    map.get_or_insert_with(0, || "rebuilt0".into());
    assert!(!map.is_array_based());
    assert_eq!(map.len(), n);
    assert_eq!(map.get(&0).map(String::as_str), Some("rebuilt0"));
    assert_eq!(keys(&map), (0..n).collect::<Vec<_>>());
}
#[test]
fn safety_ceiling_treeifies_without_churn() {
    let n = ScalingMapConfig::default().maximum_array_size + 1;
    let map = fill(n);
    assert!(!map.is_array_based());
    assert_eq!(map.len(), n);
    assert!(keys(&map).is_sorted());
    assert!(
        map.iter()
            .rev()
            .map(|(k, _)| *k)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|p| p[0] > p[1])
    );
}
#[test]
fn random_churn_matches_tree_map_model() {
    let mut map = ScalingMap::new();
    let mut oracle = BTreeMap::new();
    let mut random = SampleRandom::new(42);
    let bound = NonZeroUsize::new(2 * ScalingMapConfig::default().maximum_array_size).unwrap();
    for i in 0..100_000 {
        let key = random.index(bound);
        let value = format!("value{key}");
        match random.index(NonZeroUsize::new(10).unwrap()) {
            0..=5 => {
                map.get_or_insert_with(key, || value.clone());
                oracle.entry(key).or_insert(value);
            }
            6..=8 => {
                map.remove(&key);
                oracle.remove(&key);
            }
            _ => {
                assert_eq!(map.get(&key), oracle.get(&key));
            }
        }
        if i % 1000 == 0 {
            assert_eq!(map.len(), oracle.len());
            assert_eq!(keys(&map), oracle.keys().copied().collect::<Vec<_>>());
        }
    }
    assert_eq!(map.len(), oracle.len());
    assert_eq!(keys(&map), oracle.keys().copied().collect::<Vec<_>>());
}

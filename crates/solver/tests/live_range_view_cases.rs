// Adapted pinned Community ConnectedRangeTrackerTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
#[test]
fn hash_code_reflects_mutation() {
    let mut tree = LiveConnectedRangeCollector::<usize, i64, (i64, i64)>::default();
    tree.insert(&(0, 0, 2, (0, 2)));
    let range = tree.view(0).unwrap();
    let hash_before = hash(&range);
    tree.insert(&(1, 2, 4, (2, 4)));
    let hash_after = hash(&range);
    assert_ne!(hash_after, hash_before);
}

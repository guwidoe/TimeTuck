//! Optional immutable registration lookup; no owned/cloned entity keys.
use super::*;
use std::{
    collections::{HashMap, hash_map::RandomState},
    hash::{BuildHasher, Hash},
};
pub(super) type KeyHash<K> = fn(&RandomState, &K) -> u64;
pub(super) fn hash<K: Hash>(state: &RandomState, key: &K) -> u64 {
    state.hash_one(key)
}
struct Bucket {
    first: usize,
    last: usize,
}
pub(super) struct KeyedSourceIndex<K> {
    state: RandomState,
    hash: KeyHash<K>,
    buckets: HashMap<(usize, u64), Bucket>,
    next: Vec<usize>,
}
// Definitions are immutably borrowed from one fixed schema. Their addresses
// identify canonical fields without allocating names or changing public metadata.
fn field_id(field: PlanningVariableRef<'_>) -> usize {
    std::ptr::from_ref(field.definition()) as usize
}
impl<K: Eq> KeyedSourceIndex<K> {
    pub(super) fn new<W>(hash: KeyHash<K>, nodes: &[Node<'_, '_, K, W>]) -> Self {
        let mut index = Self {
            state: RandomState::new(),
            hash,
            buckets: HashMap::with_capacity(nodes.len()),
            next: vec![usize::MAX; nodes.len()],
        };
        for (position, node) in nodes.iter().enumerate() {
            let key = (field_id(node.variable), hash(&index.state, &node.entity));
            index
                .buckets
                .entry(key)
                .and_modify(|bucket| {
                    index.next[bucket.last] = position;
                    bucket.last = position;
                })
                .or_insert(Bucket {
                    first: position,
                    last: position,
                });
        }
        index
    }
    pub(super) fn find<W>(
        &self,
        nodes: &[Node<'_, '_, K, W>],
        field: PlanningVariableRef<'_>,
        entity: &K,
    ) -> Option<usize> {
        let key = (field_id(field), (self.hash)(&self.state, entity));
        let mut position = self.buckets.get(&key)?.first;
        while position != usize::MAX {
            let node = &nodes[position];
            if node.variable == field && &node.entity == entity {
                return Some(position);
            }
            position = self.next[position];
        }
        None
    }
}

#[cfg(test)]
#[path = "keyed_index_tests.rs"]
mod tests;

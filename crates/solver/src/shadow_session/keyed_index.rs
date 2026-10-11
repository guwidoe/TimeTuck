//! Append-only builder lookup, immutable after build; no owned/cloned entity keys.
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
            next: Vec::with_capacity(nodes.len()),
        };
        for node in nodes {
            let key = index.key(node.variable, &node.entity);
            index.insert(key);
        }
        index
    }
    pub(super) fn entity_hash(&self, entity: &K) -> u64 {
        (self.hash)(&self.state, entity)
    }
    pub(super) fn key(&self, field: PlanningVariableRef<'_>, entity: &K) -> (usize, u64) {
        (field_id(field), self.entity_hash(entity))
    }
    pub(super) fn insert(&mut self, key: (usize, u64)) {
        let position = self.next.len();
        self.next.push(usize::MAX);
        self.buckets
            .entry(key)
            .and_modify(|bucket| {
                self.next[bucket.last] = position;
                bucket.last = position;
            })
            .or_insert(Bucket {
                first: position,
                last: position,
            });
    }
    pub(super) fn find<W>(
        &self,
        nodes: &[Node<'_, '_, K, W>],
        field: PlanningVariableRef<'_>,
        entity: &K,
    ) -> Option<usize> {
        self.find_key(nodes, field, entity, self.key(field, entity))
    }
    pub(super) fn find_key<W>(
        &self,
        nodes: &[Node<'_, '_, K, W>],
        field: PlanningVariableRef<'_>,
        entity: &K,
        key: (usize, u64),
    ) -> Option<usize> {
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

//! Immutable registry and range indexes. Hash bounds stay at construction only.
use super::{ListAssignment, ListEntity, ListError, ListPosition, ListProblem};
use std::{
    collections::{HashMap, HashSet, hash_map::RandomState},
    hash::{BuildHasher, Hash},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub(super) struct ValueIndex<V> {
    hash: fn(&RandomState, &V) -> u64,
    state: RandomState,
    heads: HashMap<u64, usize>,
    next: Vec<usize>,
    ranges: Vec<Option<Vec<usize>>>,
    sparse_ranges: bool,
}
impl<V: Eq> ValueIndex<V> {
    pub(super) fn find(&self, values: &[V], value: &V) -> Option<usize> {
        let mut id = *self.heads.get(&(self.hash)(&self.state, value))?;
        while id != usize::MAX {
            if values[id] == *value {
                return Some(id);
            }
            id = self.next[id];
        }
        None
    }
    pub(super) fn accepts(&self, entity: usize, id: usize) -> bool {
        self.ranges[entity].as_ref().is_none_or(|ids| {
            if ids.len() <= 32 {
                ids.contains(&id)
            } else {
                ids.binary_search(&id).is_ok()
            }
        })
    }
}
impl<V: Eq + Clone + Hash> ListProblem<V> {
    /// Construct a model with immutable hash indexes for registry and range lookups.
    /// Useful for large registries; small models may favor [`Self::new`].
    /// Equal values must hash equally and their Eq/Hash identity must remain stable
    /// for the lifetime of the model and its clones. Hash collisions are resolved
    /// by equality. Registry order, errors, pins and solver selection order are
    /// identical to `new`. Clones share the index without cloning planning values
    /// into it. Solver APIs themselves do not require a Hash bound.
    pub fn new_indexed(values: Vec<V>, entities: Vec<ListEntity<V>>) -> Result<Self, ListError> {
        Self::new_using_hash(values, entities, |state, value| state.hash_one(value))
    }
}
impl<V: Eq + Clone> ListProblem<V> {
    fn new_using_hash(
        values: Vec<V>,
        entities: Vec<ListEntity<V>>,
        hash: fn(&RandomState, &V) -> u64,
    ) -> Result<Self, ListError> {
        let mut index = ValueIndex {
            hash,
            state: RandomState::new(),
            heads: HashMap::with_capacity(values.len()),
            next: Vec::with_capacity(values.len()),
            ranges: Vec::with_capacity(entities.len()),
            sparse_ranges: false,
        };
        for (id, value) in values.iter().enumerate() {
            if index.find(&values, value).is_some() {
                return Err(ListError::DuplicateDomainValue);
            }
            let hash = (index.hash)(&index.state, value);
            index
                .next
                .push(index.heads.insert(hash, id).unwrap_or(usize::MAX));
        }
        for (entity, meta) in entities.iter().enumerate() {
            let range = if let Some(range) = &meta.value_range {
                let mut ids = HashSet::with_capacity(range.len());
                for value in range {
                    let id = index
                        .find(&values, value)
                        .ok_or(ListError::EntityRangeValueOutsideDomain { entity })?;
                    if !ids.insert(id) {
                        return Err(ListError::DuplicateEntityRangeValue { entity });
                    }
                }
                let mut ids: Vec<_> = ids.into_iter().collect();
                ids.sort_unstable();
                Some(ids)
            } else {
                None
            };
            index.ranges.push(range);
            if meta.pinned_prefix > meta.initial.len() || (meta.pinned && meta.pinned_prefix != 0) {
                return Err(ListError::InvalidPin { entity });
            }
        }
        // A receiver index is worthwhile only for larger, mostly restricted
        // models. Compute this immutable shape hint once, not on every scan.
        let explicit = index.ranges.iter().filter_map(Option::as_ref);
        let restricted = explicit.clone().count();
        let memberships: u128 = explicit.map(|ids| ids.len() as u128).sum();
        index.sparse_ranges = entities.len() >= 32
            && values.len() >= 64
            && restricted >= entities.len().div_ceil(2)
            && memberships <= 8 * values.len() as u128;
        let model = Self {
            values,
            entities,
            value_index: Some(Arc::new(index)),
            allows_unassigned: false,
            dependencies: None,
        };
        model.validate(&model.initial_assignment())?;
        Ok(model)
    }
}

impl<V: Eq + Clone> ListProblem<V> {
    pub(crate) fn has_value_index(&self) -> bool {
        self.value_index.is_some()
    }

    pub(crate) fn has_sparse_entity_ranges(&self) -> bool {
        self.value_index
            .as_ref()
            .is_some_and(|index| index.sparse_ranges)
    }

    // Only used after has_sparse_entity_ranges: None denotes an unrestricted
    // entity. Registered IDs preserve model range membership without rehashing.
    pub(crate) fn indexed_entity_range(&self, entity: usize) -> Option<&[usize]> {
        self.value_index.as_ref().expect("indexed ranges").ranges[entity].as_deref()
    }

    // Internal callers already hold a valid registry ID; avoid rehashing its
    // value for every receiver. Eq-only models retain ordinary range membership.
    pub(crate) fn accepts_registered(&self, entity: usize, id: usize) -> bool {
        match &self.value_index {
            Some(index) => index.accepts(entity, id),
            None => self.accepts(entity, &self.values[id]),
        }
    }

    // Sparse callers can retain direct lookup; bulk callers use this only when
    // the registry supports a linear assignment traversal.
    pub(crate) fn indexed_value_locations(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Option<Vec<Option<ListPosition>>> {
        self.value_index
            .as_ref()
            .map(|_| self.value_locations(assignment))
    }

    // Callers already validated assignment membership and uniqueness.
    pub(crate) fn value_locations(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Vec<Option<ListPosition>> {
        if self.value_index.is_none() {
            return self.values.iter().map(|v| assignment.location(v)).collect();
        }
        let mut result = vec![None; self.values.len()];
        for (entity, row) in assignment.lists().iter().enumerate() {
            for (index, value) in row.iter().enumerate() {
                result[self.value_index(value).expect("validated registry")] =
                    Some(ListPosition { entity, index });
            }
        }
        result
    }
}

impl<V: Eq + Clone> ListProblem<V> {
    // Repair can change ranges; reconstruct their IDs instead of reusing stale ranges.
    pub(crate) fn rebuild_with_entities(
        &self,
        entities: Vec<ListEntity<V>>,
    ) -> Result<Self, ListError> {
        match &self.value_index {
            Some(index) => Self::new_using_hash(self.values.clone(), entities, index.hash),
            None => Self::new(self.values.clone(), entities),
        }
    }

    // Dense callers can opt into one touched-row traversal without making the
    // Eq-only fallback scan the entire registry for sparse queries.
    pub(crate) fn indexed_changed_value_positions<'a>(
        &self,
        assignment: &ListAssignment<V>,
        touched: &[usize],
        original: impl Fn(usize) -> &'a [V],
    ) -> Option<Vec<(usize, Option<ListPosition>)>>
    where
        V: 'a,
    {
        self.value_index
            .as_ref()
            .map(|_| self.changed_value_positions(assignment, touched, original))
    }

    // Preserve registry update order and include removed values. For indexed models
    // the work depends on touched rows, not the Cartesian product of registry/rows.
    pub(crate) fn changed_value_positions<'a>(
        &self,
        assignment: &ListAssignment<V>,
        touched: &[usize],
        original: impl Fn(usize) -> &'a [V],
    ) -> Vec<(usize, Option<ListPosition>)>
    where
        V: 'a,
    {
        if self.value_index.is_some() {
            let mut changes = std::collections::BTreeMap::new();
            for &entity in touched {
                for value in original(entity) {
                    changes.insert(self.value_index(value).expect("validated registry"), None);
                }
            }
            for &entity in touched {
                for (index, value) in assignment.lists()[entity].iter().enumerate() {
                    changes.insert(
                        self.value_index(value).expect("validated registry"),
                        Some(ListPosition { entity, index }),
                    );
                }
            }
            changes.into_iter().collect()
        } else {
            if touched.is_empty() {
                return Vec::new();
            }
            // Reserve from the observed edit footprint, never the whole sparse
            // registry. Removed values can exceed this estimate; Vec may grow.
            let current_len = touched.iter().fold(0usize, |len, &entity| {
                len.saturating_add(assignment.lists()[entity].len())
            });
            let capacity = self.values.len().min(current_len.saturating_mul(2));
            let mut changes = Vec::with_capacity(capacity);
            // Sparse edits should traverse their rows, not revisit those rows
            // for every registry entry. Keep the existing dense/small path;
            // temporary storage remains proportional to the edit footprint.
            if self.values.len() >= 16 && current_len <= self.values.len() / 2 {
                for &entity in touched {
                    for (index, value) in assignment.lists()[entity].iter().enumerate() {
                        changes.push((
                            self.value_index(value).expect("validated registry"),
                            Some(ListPosition { entity, index }),
                        ));
                    }
                }
                changes.sort_unstable_by_key(|(id, _)| *id);
                changes.dedup_by_key(|(id, _)| *id);
                for &entity in touched {
                    if changes.len() == self.values.len() {
                        break;
                    }
                    for value in original(entity) {
                        if !touched
                            .iter()
                            .any(|&current| assignment.lists()[current].contains(value))
                        {
                            let id = self.value_index(value).expect("validated registry");
                            if let Err(slot) = changes.binary_search_by_key(&id, |(id, _)| *id) {
                                changes.insert(slot, (id, None));
                            }
                        }
                    }
                }
                return changes;
            }
            // Original rows are frozen ListUndo lookups. Compact undo may clone
            // them on its FIRST request: keep that request at the same equality
            // short circuit, then cache only the returned immutable slice. Large
            // touched sets retain uncached lookup without an extra allocation.
            let mut originals: [Option<&'a [V]>; 8] = [None; 8];
            for (id, value) in self.values.iter().enumerate() {
                let mut position = None;
                for &entity in touched {
                    if let Some(index) = assignment.lists()[entity].iter().position(|v| v == value)
                    {
                        position = Some(ListPosition { entity, index });
                        break;
                    }
                }
                let mut changed = position.is_some();
                if !changed {
                    for (slot, &entity) in touched.iter().enumerate() {
                        let row = if touched.len() <= originals.len() {
                            *originals[slot].get_or_insert_with(|| original(entity))
                        } else {
                            original(entity)
                        };
                        if row.contains(value) {
                            changed = true;
                            break;
                        }
                    }
                }
                if changed {
                    changes.push((id, position));
                }
            }
            changes
        }
    }
}

// Pass-local evidence: an absent entry in Changed means unknown, whereas a
// present None proves unassignment after the complete composite transaction.
pub(crate) enum ValueLocations {
    Full(Vec<Option<ListPosition>>),
    Changed(Vec<(usize, Option<ListPosition>)>),
}
impl ValueLocations {
    #[inline]
    pub(crate) fn get(&self, id: usize) -> Option<Option<ListPosition>> {
        match self {
            Self::Full(positions) => Some(positions[id]),
            Self::Changed(positions) => positions
                .binary_search_by_key(&id, |(id, _)| *id)
                .ok()
                .map(|index| positions[index].1),
        }
    }
}

// Validation chooses storage from immutable shape and observed row lengths.
// Sparse registries must not allocate in proportion to the whole domain.
pub(super) enum AssignedIds {
    Sparse(HashSet<usize>),
    Dense(Vec<u64>),
    Positions(Vec<Option<ListPosition>>),
}
impl AssignedIds {
    pub(super) fn new<V>(count: usize, assignment: &ListAssignment<V>, positions: bool) -> Self {
        if count >= 64 {
            let threshold = count.div_ceil(8);
            let mut assigned = 0usize;
            for row in assignment.lists() {
                assigned = assigned.saturating_add(row.len());
                if assigned >= threshold {
                    return if positions {
                        Self::Positions(vec![None; count])
                    } else {
                        Self::Dense(vec![0; count.div_ceil(64)])
                    };
                }
            }
        }
        Self::Sparse(HashSet::new())
    }
    pub(super) fn insert(&mut self, id: usize, position: ListPosition) -> bool {
        match self {
            Self::Sparse(seen) => seen.insert(id),
            Self::Dense(words) => {
                let mask = 1u64 << (id % 64);
                let word = &mut words[id / 64];
                let unseen = *word & mask == 0;
                *word |= mask;
                unseen
            }
            Self::Positions(positions) => positions[id].replace(position).is_none(),
        }
    }
    pub(super) fn into_positions(self) -> Option<Vec<Option<ListPosition>>> {
        match self {
            Self::Positions(p) => Some(p),
            _ => None,
        }
    }
}

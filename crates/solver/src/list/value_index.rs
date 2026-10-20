//! Immutable registry/range indexes with explicit hash or checked ordinal lookup.
//! Hash bounds stay at hash construction only; ordinary Eq-only defaults remain.
use super::{ListAssignment, ListEntity, ListError, ListPosition, ListProblem};
use std::{
    collections::{HashMap, HashSet, hash_map::RandomState},
    hash::{BuildHasher, Hash},
    sync::Arc,
};

#[derive(Clone, Debug)]
enum RegistryLookup<V> {
    Hash {
        hash: fn(&RandomState, &V) -> u64,
        state: RandomState,
        heads: HashMap<u64, usize>,
        next: Vec<usize>,
    },
    Ordinal(fn(&V) -> usize),
}
#[derive(Clone, Debug)]
pub(super) struct ValueIndex<V> {
    lookup: RegistryLookup<V>,
    ranges: Vec<Option<Vec<usize>>>,
    sparse_ranges: bool,
}
impl<V: Eq> ValueIndex<V> {
    pub(super) fn find(&self, values: &[V], value: &V) -> Option<usize> {
        match &self.lookup {
            RegistryLookup::Hash {
                hash,
                state,
                heads,
                next,
            } => {
                let mut id = *heads.get(&hash(state, value))?;
                while id != usize::MAX {
                    if values[id] == *value {
                        return Some(id);
                    }
                    id = next[id];
                }
                None
            }
            RegistryLookup::Ordinal(ordinal) => {
                let id = ordinal(value);
                values
                    .get(id)
                    .filter(|registered| **registered == *value)
                    .map(|_| id)
            }
        }
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
            lookup: RegistryLookup::Hash {
                hash,
                state: RandomState::new(),
                heads: HashMap::with_capacity(values.len()),
                next: Vec::with_capacity(values.len()),
            },
            ranges: Vec::with_capacity(entities.len()),
            sparse_ranges: false,
        };
        for (id, value) in values.iter().enumerate() {
            if index.find(&values, value).is_some() {
                return Err(ListError::DuplicateDomainValue);
            }
            let RegistryLookup::Hash {
                hash,
                state,
                heads,
                next,
            } = &mut index.lookup
            else {
                unreachable!("hash construction")
            };
            let key = hash(state, value);
            next.push(heads.insert(key, id).unwrap_or(usize::MAX));
        }
        Self::finish_indexed(values, entities, index)
    }

    /// Construct an immutable direct ordinal registry index without requiring Hash.
    /// Every registry value must map to its own position in `values`, independent
    /// of numeric value or sorting. Equal values must have equal ordinals, and
    /// Eq/ordinal identity must remain stable for the model and its clones.
    /// The callback must be pure; invalid callback traces are not equivalent to
    /// ordinary equality lookup. Out-of-range/unequal queries never match: each
    /// lookup checks both bounds and Eq. Duplicate values mapping to an earlier
    /// equal entry report DuplicateDomainValue; other bad registry ordinals report
    /// InvalidValueOrdinal. Registry/update/selection order, pins and ranges remain
    /// unchanged for valid keys. Clones share the immutable index. Rebuilds retain
    /// this exact function. No default constructor or solver Hash bound changes.
    pub fn new_ordinal_indexed(
        values: Vec<V>,
        entities: Vec<ListEntity<V>>,
        ordinal: fn(&V) -> usize,
    ) -> Result<Self, ListError> {
        for (expected, value) in values.iter().enumerate() {
            let actual = ordinal(value);
            if actual < expected && values.get(actual).is_some_and(|earlier| earlier == value) {
                return Err(ListError::DuplicateDomainValue);
            }
            if actual != expected {
                return Err(ListError::InvalidValueOrdinal { expected, actual });
            }
        }
        let index = ValueIndex {
            lookup: RegistryLookup::Ordinal(ordinal),
            ranges: Vec::with_capacity(entities.len()),
            sparse_ranges: false,
        };
        Self::finish_indexed(values, entities, index)
    }

    fn finish_indexed(
        values: Vec<V>,
        entities: Vec<ListEntity<V>>,
        mut index: ValueIndex<V>,
    ) -> Result<Self, ListError> {
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
            Some(index) => match &index.lookup {
                RegistryLookup::Hash { hash, .. } => {
                    Self::new_using_hash(self.values.clone(), entities, *hash)
                }
                RegistryLookup::Ordinal(ordinal) => {
                    Self::new_ordinal_indexed(self.values.clone(), entities, *ordinal)
                }
            },
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
        if let Some(index) = &self.value_index {
            if matches!(index.lookup, RegistryLookup::Ordinal(_)) {
                // Bounded registries need no tree or sorting. Every ordinal
                // still checks bounds and equality before becoming a bit/slot.
                // Original captures precede current rows; current owners win.
                if self.values.len() <= 64 {
                    let mut positions = [None; 64];
                    let mut changed = 0u64;
                    for &entity in touched {
                        for value in original(entity) {
                            let id = index.find(&self.values, value).expect("validated registry");
                            changed |= 1u64 << id;
                        }
                    }
                    for &entity in touched {
                        for (slot, value) in assignment.lists()[entity].iter().enumerate() {
                            let id = index.find(&self.values, value).expect("validated registry");
                            changed |= 1u64 << id;
                            positions[id] = Some(ListPosition {
                                entity,
                                index: slot,
                            });
                        }
                    }
                    let mut changes = Vec::with_capacity(changed.count_ones() as usize);
                    while changed != 0 {
                        let id = changed.trailing_zeros() as usize;
                        changes.push((id, positions[id]));
                        changed &= changed - 1;
                    }
                    return changes;
                }
                // An explicit ordinal index permits checked, direct IDs. Keep
                // captures in one footprint-sized buffer instead of allocating
                // a tree node for each affected value. Current positions win
                // over removals; validated assignments have one current owner.
                let current_len = touched.iter().fold(0usize, |len, &entity| {
                    len.saturating_add(assignment.lists()[entity].len())
                });
                let capacity = self.values.len().min(current_len.saturating_mul(2));
                let mut changes = Vec::with_capacity(capacity);
                for &entity in touched {
                    for value in original(entity) {
                        changes.push((
                            index.find(&self.values, value).expect("validated registry"),
                            None,
                        ));
                    }
                }
                for &entity in touched {
                    for (slot, value) in assignment.lists()[entity].iter().enumerate() {
                        changes.push((
                            index.find(&self.values, value).expect("validated registry"),
                            Some(ListPosition {
                                entity,
                                index: slot,
                            }),
                        ));
                    }
                }
                changes.sort_unstable_by_key(|(id, position)| (*id, position.is_none()));
                changes.dedup_by_key(|(id, _)| *id);
                return changes;
            }
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

#[cfg(test)]
mod ordinal_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    fn ordinal(v: &usize) -> usize {
        CALLS.fetch_add(1, Ordering::Relaxed);
        *v
    }
    #[test]
    fn clone_shares_index_and_rebuild_retains_ordinal_function_and_recomputes_ranges() {
        let p = ListProblem::new_ordinal_indexed(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![0]).with_value_range(vec![0, 1])],
            ordinal,
        )
        .unwrap();
        let q = p.clone();
        assert!(Arc::ptr_eq(
            p.value_index.as_ref().unwrap(),
            q.value_index.as_ref().unwrap()
        ));
        let r = q
            .rebuild_with_entities(vec![ListEntity::new(vec![2]).with_value_range(vec![2])])
            .unwrap();
        assert!(matches!(
            &r.value_index.as_ref().unwrap().lookup,
            RegistryLookup::Ordinal(_)
        ));
        CALLS.store(0, Ordering::Relaxed);
        assert_eq!(r.value_index(&2), Some(2));
        assert_eq!(CALLS.load(Ordering::Relaxed), 1);
        assert!(!r.accepts_registered(0, 0));
        assert!(r.accepts_registered(0, 2));
        assert!(!p.accepts_registered(0, 2));
        assert_eq!(r.value_index(&usize::MAX), None);
    }
    #[test]
    fn ordinal_changed_positions_keep_registry_order_current_owners_and_removals() {
        for count in [64, 65, 4096] {
            let p = ListProblem::new_ordinal_indexed(
                (0..count).collect(),
                vec![
                    ListEntity::new(vec![31, 0, 1]),
                    ListEntity::new(vec![63, 32]),
                    ListEntity::new(vec![2]),
                ],
                |v| *v,
            )
            .unwrap()
            .allowing_unassigned();
            let before = p.initial_assignment();
            let after = ListAssignment::new(vec![vec![32, 0], vec![31, 3], vec![2]]);
            p.validate(&after).unwrap();
            let changes =
                p.changed_value_positions(&after, &[1, 0], |entity| &before.lists()[entity]);
            assert_eq!(
                changes,
                vec![
                    (
                        0,
                        Some(ListPosition {
                            entity: 0,
                            index: 1
                        })
                    ),
                    (1, None),
                    (
                        3,
                        Some(ListPosition {
                            entity: 1,
                            index: 1
                        })
                    ),
                    (
                        31,
                        Some(ListPosition {
                            entity: 1,
                            index: 0
                        })
                    ),
                    (
                        32,
                        Some(ListPosition {
                            entity: 0,
                            index: 0
                        })
                    ),
                    (63, None)
                ]
            );
            assert!(
                p.changed_value_positions(&after, &[], |entity| &before.lists()[entity])
                    .is_empty()
            );
            let removed = ListAssignment::new(vec![vec![], vec![], vec![2]]);
            assert_eq!(
                p.changed_value_positions(&removed, &[0, 1], |entity| &before.lists()[entity]),
                vec![(0, None), (1, None), (31, None), (32, None), (63, None)]
            );
        }
    }
    #[test]
    fn hash_rebuild_stays_hash_and_ordinal_sparse_shape_preserves_sorted_range_ids() {
        let p = ListProblem::new_indexed(vec![0, 1], vec![ListEntity::new(vec![0])]).unwrap();
        assert!(matches!(
            &p.rebuild_with_entities(vec![ListEntity::new(vec![1])])
                .unwrap()
                .value_index
                .as_ref()
                .unwrap()
                .lookup,
            RegistryLookup::Hash { .. }
        ));
        let p = ListProblem::new_ordinal_indexed(
            (0..64).collect(),
            (0..32)
                .map(|i| ListEntity::new(vec![i]).with_value_range(vec![i + 32, i]))
                .collect(),
            |v| *v,
        )
        .unwrap();
        assert!(p.has_sparse_entity_ranges());
        assert_eq!(p.indexed_entity_range(7), Some(&[7, 39][..]));
    }
}

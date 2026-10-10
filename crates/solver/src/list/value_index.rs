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
        self.ranges[entity]
            .as_ref()
            .is_none_or(|ids| ids.binary_search(&id).is_ok())
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
            self.values
                .iter()
                .enumerate()
                .filter_map(|(id, value)| {
                    let position = touched.iter().find_map(|&entity| {
                        assignment.lists()[entity]
                            .iter()
                            .position(|v| v == value)
                            .map(|index| ListPosition { entity, index })
                    });
                    (position.is_some()
                        || touched
                            .iter()
                            .any(|&entity| original(entity).contains(value)))
                    .then_some((id, position))
                })
                .collect()
        }
    }
}

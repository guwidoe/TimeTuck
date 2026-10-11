//! Optional immutable domain membership; ordinary model APIs retain Eq bounds.
use super::{FiniteProblem, ModelError, Variable};
use std::{
    collections::{HashMap, hash_map::RandomState},
    hash::{BuildHasher, Hash, Hasher},
    sync::Arc,
};

pub(crate) type TupleHash<V> = fn(&RandomState, &[Option<V>], &[usize]) -> u64;

impl<V> FiniteProblem<V> {
    pub(crate) fn grouping_hash(&self) -> Option<TupleHash<V>> {
        self.grouping_hash
    }
}

#[derive(Clone, Debug)]
pub(super) struct DomainIndex<V> {
    hash: fn(&RandomState, &V) -> u64,
    state: RandomState,
    heads: HashMap<u64, usize>,
    next: Vec<usize>,
}
impl<V: Eq> DomainIndex<V> {
    fn contains(&self, values: &[V], value: &V) -> bool {
        self.contains_hashed(values, value, (self.hash)(&self.state, value))
    }
    fn contains_hashed(&self, values: &[V], value: &V, hash: u64) -> bool {
        let Some(&head) = self.heads.get(&hash) else {
            return false;
        };
        let mut id = head;
        while id != usize::MAX {
            if values[id] == *value {
                return true;
            }
            id = self.next[id];
        }
        false
    }
}
impl<V: Eq + Hash> DomainIndex<V> {
    fn new(values: &[V]) -> Result<Option<Self>, ()> {
        let mut index = Self {
            hash: |s, v| s.hash_one(v),
            state: RandomState::new(),
            heads: HashMap::with_capacity(values.len()),
            next: Vec::with_capacity(values.len()),
        };
        for (id, value) in values.iter().enumerate() {
            let h = (index.hash)(&index.state, value);
            let mut previous = index.heads.get(&h).copied().unwrap_or(usize::MAX);
            let mut collisions = 0;
            while previous != usize::MAX {
                if values[previous] == *value {
                    return Err(());
                }
                collisions += 1;
                // A poor hash should not turn direct membership into pointer
                // chasing. The caller rechecks metadata using direct equality.
                if collisions >= 8 {
                    return Ok(None);
                }
                previous = index.next[previous];
            }
            index
                .next
                .push(index.heads.insert(h, id).unwrap_or(usize::MAX));
        }
        Ok(Some(index))
    }
}
impl<V: Eq> Variable<V> {
    #[inline]
    pub(crate) fn contains_value(&self, value: &V) -> bool {
        match &self.domain_index {
            Some(index) => index.contains(&self.domain, value),
            None => self.domain.contains(value),
        }
    }
    pub(super) fn validate_metadata(&self, variable: usize) -> Result<(), ModelError> {
        // An installed index was built from this immutable domain and already
        // checked duplicates. Small and Eq-only ranges retain direct scans.
        if self.domain_index.is_none() {
            for (i, value) in self.domain.iter().enumerate() {
                if self.domain[..i].contains(value) {
                    return Err(ModelError::DuplicateValue { variable });
                }
            }
        }
        if let Some(value) = &self.initial
            && !self.contains_value(value)
        {
            return Err(self.range_error(variable, value));
        }
        Ok(())
    }
}
impl<V: Eq + Hash> FiniteProblem<V> {
    /// Construct a model with immutable membership indexes for large domains.
    /// Domain order, diagnostics, pins and solver selection order match [`Self::new`].
    /// Domains shorter than 64 values or with long hash-collision chains retain
    /// direct lookup to avoid index overhead.
    /// Model clones and repair models share indexes without cloning values into them.
    /// Large pillar groupings may also hash current value tuples; small groupings
    /// and poor hashes retain direct equality checks. Group/member order is unchanged.
    ///
    /// Equal values must hash equally, and Eq/Hash identities must remain stable
    /// for the lifetime of the model and its clones. Hash collisions use equality.
    /// Solver and move APIs retain their ordinary Eq bounds; indexing is opt-in.
    ///
    /// ```
    /// use timetuck::*;
    /// let model = FiniteProblem::new_indexed(vec![
    ///     Variable::new((0..1000).collect()).with_initial(999),
    /// ])?;
    /// let mut assignment = model.initial_assignment();
    /// model.apply(&mut assignment, &Move::Change { variable: 0, value: Some(42) })?.commit();
    /// assert_eq!(assignment.values(), &[Some(42)]);
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn new_indexed(mut variables: Vec<Variable<V>>) -> Result<Self, ModelError> {
        for (i, variable) in variables.iter_mut().enumerate() {
            variable.domain_index = if variable.domain.len() >= 64 {
                DomainIndex::new(&variable.domain)
                    .map_err(|()| ModelError::DuplicateValue { variable: i })?
                    .map(Arc::new)
            } else {
                None
            };
            variable.validate_metadata(i)?;
        }
        Ok(Self {
            variables,
            pillar_schema: None,
            grouping_hash: Some(|state, values, slots| {
                let mut hasher = state.build_hasher();
                slots.len().hash(&mut hasher);
                for &slot in slots {
                    values[slot].hash(&mut hasher);
                }
                hasher.finish()
            }),
        })
    }
}

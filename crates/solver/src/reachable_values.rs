//! Immutable inverse-domain indexes for candidate reachability.
use crate::{FiniteProblem, ListProblem};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReachabilityError<E> {
    DuplicateEntity(E),
}
impl<E: fmt::Debug> fmt::Display for ReachabilityError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duplicate reachability entity: {self:?}")
    }
}
impl<E: fmt::Debug> Error for ReachabilityError<E> {}

#[derive(Clone)]
struct Domain<V> {
    values: Arc<[V]>,
    optional: bool,
}
struct Index<E, V> {
    domains: BTreeMap<E, Domain<V>>,
    entities: BTreeMap<V, Arc<[E]>>,
}
/// Captured domain membership, independent of current assignment, pins or list
/// positions. It identifies candidate relationships, not complete move legality.
/// Values/entities use their Ord identity; duplicate values in one input range
/// collapse. Equal ranges share storage, but every distinct entity remains in the
/// inverse index. Input ranges are already materialized finite vectors.
///
/// All results have deterministic key order. co_values excludes the origin and
/// excludes unassignment. Optional reachability requires a shared optional domain;
/// an unknown origin cannot reach None. None-to-None and value-to-self are no-ops
/// and not reachable. Different comparator views never mutate the stored index.
pub struct ReachableValues<E, V> {
    index: Arc<Index<E, V>>,
}
impl<E, V> Clone for ReachableValues<E, V> {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
        }
    }
}
impl<E: Ord + Clone, V: Ord + Clone> ReachableValues<E, V> {
    /// Each tuple is (entity, finite values, allows_unassigned).
    pub fn new(ranges: Vec<(E, Vec<V>, bool)>) -> Result<Self, ReachabilityError<E>> {
        let mut domains = BTreeMap::new();
        let mut canonical: BTreeMap<Vec<V>, Arc<[V]>> = BTreeMap::new();
        let mut inverse: BTreeMap<V, Vec<E>> = BTreeMap::new();
        for (entity, values, optional) in ranges {
            if domains.contains_key(&entity) {
                return Err(ReachabilityError::DuplicateEntity(entity));
            }
            let values: Vec<V> = values
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let shared = canonical
                .entry(values.clone())
                .or_insert_with(|| values.into())
                .clone();
            for value in shared.iter() {
                inverse
                    .entry(value.clone())
                    .or_default()
                    .push(entity.clone());
            }
            domains.insert(
                entity,
                Domain {
                    values: shared,
                    optional,
                },
            );
        }
        let entities = inverse
            .into_iter()
            .map(|(v, mut entities)| {
                entities.sort();
                (v, entities.into())
            })
            .collect();
        Ok(Self {
            index: Arc::new(Index { domains, entities }),
        })
    }
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.index.entities.keys()
    }
    pub fn entities(&self, value: &V) -> &[E] {
        self.index.entities.get(value).map_or(&[], |v| v.as_ref())
    }
    pub fn domain(&self, entity: &E) -> Option<Arc<[V]>> {
        self.index.domains.get(entity).map(|d| d.values.clone())
    }
    pub fn entity_accepts(&self, entity: &E, value: Option<&V>) -> bool {
        self.index
            .domains
            .get(entity)
            .is_some_and(|d| value.map_or(d.optional, |v| d.values.binary_search(v).is_ok()))
    }
    pub fn is_entity_reachable(&self, value: &V, entity: &E) -> bool {
        self.entity_accepts(entity, Some(value))
    }
    pub fn co_values(&self, origin: &V) -> Vec<&V> {
        let mut values = BTreeSet::new();
        for entity in self.entities(origin) {
            for value in self.index.domains[entity].values.iter() {
                if value != origin {
                    values.insert(value);
                }
            }
        }
        values.into_iter().collect()
    }
    pub fn co_values_by(&self, origin: &V, mut compare: impl FnMut(&V, &V) -> Ordering) -> Vec<&V> {
        let mut values = self.co_values(origin);
        values.sort_by(|a, b| compare(a, b));
        values
    }
    pub fn is_value_reachable(&self, origin: Option<&V>, target: Option<&V>) -> bool {
        if origin == target {
            return false;
        }
        match (origin, target) {
            (Some(origin), target) => self
                .entities(origin)
                .iter()
                .any(|entity| self.entity_accepts(entity, target)),
            (None, Some(target)) => self
                .entities(target)
                .iter()
                .any(|entity| self.index.domains[entity].optional),
            (None, None) => false,
        }
    }
    /// All structurally reachable destinations; None appears first when reachable.
    pub fn destinations(&self, origin: Option<&V>) -> Vec<Option<&V>> {
        let mut result = vec![];
        if self.is_value_reachable(origin, None) {
            result.push(None);
        }
        match origin {
            Some(origin) => result.extend(self.co_values(origin).into_iter().map(Some)),
            None => result.extend(
                self.values()
                    .filter(|v| self.is_value_reachable(None, Some(v)))
                    .map(Some),
            ),
        }
        result
    }
}

/// Capture native variable domains; each variable is a distinct indexed entity.
/// Pins intentionally do not erase structural membership. Providers still apply
/// their ordinary pin and no-op rules before submitting a move.
pub fn basic_reachability<V: Ord + Clone>(problem: &FiniteProblem<V>) -> ReachableValues<usize, V> {
    ReachableValues::new(
        problem
            .variables()
            .iter()
            .enumerate()
            .map(|(entity, v)| (entity, v.domain().to_vec(), v.allows_unassigned()))
            .collect(),
    )
    .expect("variable IDs are unique")
}
/// Capture native list-owner ranges, expanding unrestricted owners to the problem
/// value universe. List unassignment follows the model-wide optional setting.
pub fn list_reachability<V: Ord + Clone>(problem: &ListProblem<V>) -> ReachableValues<usize, V> {
    ReachableValues::new(
        problem
            .entities()
            .iter()
            .enumerate()
            .map(|(entity, v)| {
                (
                    entity,
                    v.value_range().unwrap_or(problem.values()).to_vec(),
                    problem.allows_unassigned(),
                )
            })
            .collect(),
    )
    .expect("list entity IDs are unique")
}

//! Transactional, change-sensitive derived values along planning lists.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, SnapshotMap};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    ops::Deref,
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListCascadeError<V> {
    Assignment(ListError),
    UnknownValue(V),
    DuplicateSeed(V),
}
impl<V: fmt::Debug> fmt::Display for ListCascadeError<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assignment(e) => write!(f, "invalid cascade assignment: {e}"),
            Self::UnknownValue(v) => write!(f, "unknown cascade value: {v:?}"),
            Self::DuplicateSeed(v) => write!(f, "duplicate cascade seed: {v:?}"),
        }
    }
}
impl<V: fmt::Debug> Error for ListCascadeError<V> {}
impl<V> From<ListError> for ListCascadeError<V> {
    fn from(value: ListError) -> Self {
        Self::Assignment(value)
    }
}

/// Which neighbor supplies an already updated shadow. Previous propagates from
/// the head to the tail; Next propagates from the tail to the head. Independent
/// caches can calculate both directions over the same assignment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ListCascadeDirection {
    #[default]
    Previous,
    Next,
}

/// Inputs to one cascade calculation. The selected neighbor shadow is already
/// updated; the other direction's shadow is None. `current` is this element's
/// previous cached result. Unassigned values have no position or neighbors.
/// A callback may read immutable facts and these inputs; it must not depend on
/// other cached shadows or change its formula without explicit invalidation.
/// Recalculation must return the same result when its logical inputs are unchanged;
/// `current` permits preserving an existing value, not accumulating call counts.
pub struct ListCascadeContext<'a, V, S> {
    pub value: &'a V,
    pub position: Option<ListPosition>,
    pub previous: Option<&'a V>,
    pub next: Option<&'a V>,
    pub previous_shadow: Option<&'a S>,
    /// Populated only for Next cascades; previous_shadow is then None.
    pub next_shadow: Option<&'a S>,
    pub current: Option<&'a S>,
}

/// One independently propagated cascade, or several jointly updated fields in S.
///
/// Updates compare owner/index/neighbor identities and explicitly dirty values.
/// Calculations run in dependency order at most once per value, propagating
/// in the selected direction only while their Option<S> result changes.
/// Independently dirty values still run after an earlier cascade stops. Use separate caches for independent
/// callbacks; use a tuple/struct for fields updated together by one callback.
///
/// List validation and structural change detection inspect the full assignment;
/// callback work is selective. This is not a claim of constant-time list editing.
/// Clones share immutable assignment and persistent shadow storage, even when S
/// is not Clone. Metadata errors and callback unwinding leave the cache unchanged;
/// external callback side effects cannot be rolled back.
///
/// ```
/// use timetuck::*;
/// let problem = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])?;
/// let cache = ListCascadeCache::new(&problem, &problem.initial_assignment(), |c| {
///     c.position.map(|_| c.previous_shadow.copied().unwrap_or(0) + *c.value)
/// })?;
/// assert_eq!(cache.shadows()[&3], Some(6));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ListCascadeCache<V, S> {
    direction: ListCascadeDirection,
    problem: Arc<ListProblem<V>>,
    assignment: Arc<ListAssignment<V>>,
    positions: Arc<BTreeMap<V, ListPosition>>,
    shadows: SnapshotMap<V, Option<S>>,
    pending: Arc<BTreeSet<V>>,
}
impl<V, S> Clone for ListCascadeCache<V, S> {
    fn clone(&self) -> Self {
        Self {
            direction: self.direction,
            problem: self.problem.clone(),
            assignment: self.assignment.clone(),
            positions: self.positions.clone(),
            shadows: self.shadows.clone(),
            pending: self.pending.clone(),
        }
    }
}
fn positions<V: Ord + Clone>(a: &ListAssignment<V>) -> BTreeMap<V, ListPosition> {
    a.lists()
        .iter()
        .enumerate()
        .flat_map(|(entity, list)| {
            list.iter()
                .enumerate()
                .map(move |(index, v)| (v.clone(), ListPosition { entity, index }))
        })
        .collect()
}
fn neighbors<V>(a: &ListAssignment<V>, position: Option<ListPosition>) -> (Option<&V>, Option<&V>) {
    position.map_or((None, None), |p| {
        let list = &a.lists()[p.entity];
        (
            p.index.checked_sub(1).and_then(|i| list.get(i)),
            list.get(p.index + 1),
        )
    })
}
struct CascadeUpdate<V> {
    roots: BTreeSet<V>,
    positions: BTreeMap<V, ListPosition>,
}
impl<V: Ord + Clone, S: Eq> ListCascadeCache<V, S> {
    /// Calculate all initial shadows, including unassigned values, exactly once.
    pub fn new(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        evaluate: impl FnMut(ListCascadeContext<'_, V, S>) -> Option<S>,
    ) -> Result<Self, ListCascadeError<V>> {
        Self::new_with_direction(
            problem,
            assignment,
            ListCascadeDirection::Previous,
            evaluate,
        )
    }
    /// Initialize a cascade in dependency order for the selected neighbor.
    pub fn new_with_direction(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        direction: ListCascadeDirection,
        evaluate: impl FnMut(ListCascadeContext<'_, V, S>) -> Option<S>,
    ) -> Result<Self, ListCascadeError<V>> {
        let mut cache =
            Self::from_values_with_direction(problem, assignment, direction, std::iter::empty())?;
        cache.update(assignment, problem.values(), evaluate)?;
        Ok(cache)
    }
    /// Import existing shadows without invoking a callback. Unspecified values
    /// start at None. This deliberately permits partially initialized snapshots:
    /// callers must explicitly dirty any imported value requiring recalculation.
    pub fn from_values(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        values: impl IntoIterator<Item = (V, S)>,
    ) -> Result<Self, ListCascadeError<V>> {
        Self::from_values_with_direction(
            problem,
            assignment,
            ListCascadeDirection::Previous,
            values,
        )
    }
    /// Import seeds for the selected direction, without evaluating them. As with
    /// from_values, callers must explicitly dirty seeds requiring recalculation.
    pub fn from_values_with_direction(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        direction: ListCascadeDirection,
        values: impl IntoIterator<Item = (V, S)>,
    ) -> Result<Self, ListCascadeError<V>> {
        problem.validate(assignment)?;
        let mut shadows = SnapshotMap::new();
        for value in problem.values() {
            shadows.insert(value.clone(), None);
        }
        let mut seen = BTreeSet::new();
        for (value, shadow) in values {
            if !shadows.contains_key(&value) {
                return Err(ListCascadeError::UnknownValue(value));
            }
            if !seen.insert(value.clone()) {
                return Err(ListCascadeError::DuplicateSeed(value));
            }
            shadows.insert(value, Some(shadow));
        }
        Ok(Self {
            direction,
            problem: Arc::new(problem.clone()),
            assignment: Arc::new(assignment.clone()),
            positions: Arc::new(positions(assignment)),
            shadows,
            pending: Arc::new(BTreeSet::new()),
        })
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.assignment
    }
    pub fn direction(&self) -> ListCascadeDirection {
        self.direction
    }
    pub fn shadows(&self) -> &SnapshotMap<V, Option<S>> {
        &self.shadows
    }
    /// Load without calculating shadows, scheduling every value for initialization.
    /// Existing values may instead be imported with `from_values`, followed by
    /// `queue_update` with explicit dirty roots.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])])?;
    /// let mut c = ListCascadeCache::<_, i64>::new_deferred(&p, &p.initial_assignment())?;
    /// assert_eq!(c.pending_updates().len(), 2);
    /// assert_eq!(c.shadows()[&1], None);
    /// c.flush_pending(|v| Some(v.previous_shadow.copied().unwrap_or(0) + i64::from(*v.value)))?;
    /// assert_eq!(c.shadows()[&2], Some(3));
    /// assert_eq!(c.pending_updates().len(), 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new_deferred(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<Self, ListCascadeError<V>> {
        Self::new_deferred_with_direction(problem, assignment, ListCascadeDirection::Previous)
    }
    /// Queue initialization without evaluating either neighbor direction.
    pub fn new_deferred_with_direction(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        direction: ListCascadeDirection,
    ) -> Result<Self, ListCascadeError<V>> {
        let mut cache =
            Self::from_values_with_direction(problem, assignment, direction, std::iter::empty())?;
        cache.pending = Arc::new(problem.values().iter().cloned().collect());
        Ok(cache)
    }
    /// Queued roots in value order. A root may also cause downstream propagation;
    /// this is not the number of callbacks that the next flush will execute.
    pub fn pending_updates(&self) -> std::collections::btree_set::Iter<'_, V> {
        self.pending.iter()
    }
    /// Discard queued roots, retaining the latest assignment and stored shadows.
    /// Stored shadows may deliberately remain stale or uninitialized. To repair
    /// all of them later, queue every problem value explicitly before flushing.
    pub fn clear_pending_updates(&mut self) -> usize {
        let count = self.pending.len();
        self.pending = Arc::new(BTreeSet::new());
        count
    }
    /// Install a validated assignment without calculating shadows. Structural
    /// changes and explicit dirty roots accumulate across calls, deduplicated.
    /// Flushing uses the latest assignment; intermediate states are not replayed.
    /// Invalid input or unwinding during preparation leaves all state unchanged.
    pub fn queue_update(
        &mut self,
        assignment: &ListAssignment<V>,
        dirty: &[V],
    ) -> Result<(), ListCascadeError<V>> {
        let CascadeUpdate { roots, positions } = self.prepare_update(assignment, dirty)?;
        let assignment = Arc::new(assignment.clone());
        self.assignment = assignment;
        self.positions = Arc::new(positions);
        self.pending = Arc::new(roots);
        Ok(())
    }
    /// Calculate pending roots and their affected dependents at most once each.
    /// A successful flush empties the queue; errors or callback unwinding retain
    /// both the queue and every stored shadow so the operation can be retried.
    pub fn flush_pending(
        &mut self,
        evaluate: impl FnMut(ListCascadeContext<'_, V, S>) -> Option<S>,
    ) -> Result<Vec<V>, ListCascadeError<V>> {
        let assignment = self.assignment.clone();
        self.update(&assignment, &[], evaluate)
    }
    fn prepare_update(
        &self,
        assignment: &ListAssignment<V>,
        dirty: &[V],
    ) -> Result<CascadeUpdate<V>, ListCascadeError<V>> {
        self.problem.validate(assignment)?;
        let mut roots = (*self.pending).clone();
        for value in dirty {
            if !self.shadows.contains_key(value) {
                return Err(ListCascadeError::UnknownValue(value.clone()));
            }
            roots.insert(value.clone());
        }
        let next_positions = positions(assignment);
        for value in self.problem.values() {
            let before = self.positions.get(value).copied();
            let after = next_positions.get(value).copied();
            if before != after
                || neighbors(&self.assignment, before) != neighbors(assignment, after)
            {
                roots.insert(value.clone());
            }
        }
        Ok(CascadeUpdate {
            roots,
            positions: next_positions,
        })
    }
    /// Install a validated assignment and propagate structural/explicit changes.
    /// Pending roots are included and cleared only on success.
    /// Return exactly the changed shadow keys, in key order. Mark values reading
    /// changed external facts dirty even when their list metadata did not change.
    pub fn update(
        &mut self,
        assignment: &ListAssignment<V>,
        dirty: &[V],
        mut evaluate: impl FnMut(ListCascadeContext<'_, V, S>) -> Option<S>,
    ) -> Result<Vec<V>, ListCascadeError<V>> {
        let CascadeUpdate {
            roots,
            positions: next_positions,
        } = self.prepare_update(assignment, dirty)?;
        let mut shadows = self.shadows.clone();
        let mut changed = BTreeSet::new();
        // Unassigned values have no predecessors or consumers in a list.
        for value in self
            .problem
            .values()
            .iter()
            .filter(|v| !next_positions.contains_key(*v))
        {
            if !roots.contains(value) {
                continue;
            }
            let result = evaluate(ListCascadeContext {
                value,
                position: None,
                previous: None,
                next: None,
                previous_shadow: None,
                next_shadow: None,
                current: shadows[value].as_ref(),
            });
            if shadows[value] != result {
                shadows.insert(value.clone(), result);
                changed.insert(value.clone());
            }
        }
        for (entity, list) in assignment.lists().iter().enumerate() {
            let mut propagated = false;
            for offset in 0..list.len() {
                let index = match self.direction {
                    ListCascadeDirection::Previous => offset,
                    ListCascadeDirection::Next => list.len() - 1 - offset,
                };
                let value = &list[index];
                if !propagated && !roots.contains(value) {
                    continue;
                }
                let previous = index.checked_sub(1).and_then(|i| list.get(i));
                let next = list.get(index + 1);
                let result = evaluate(ListCascadeContext {
                    value,
                    position: Some(ListPosition { entity, index }),
                    previous,
                    next,
                    previous_shadow: (self.direction == ListCascadeDirection::Previous)
                        .then(|| previous.and_then(|v| shadows[v].as_ref()))
                        .flatten(),
                    next_shadow: (self.direction == ListCascadeDirection::Next)
                        .then(|| next.and_then(|v| shadows[v].as_ref()))
                        .flatten(),
                    current: shadows[value].as_ref(),
                });
                propagated = shadows[value] != result;
                if propagated {
                    shadows.insert(value.clone(), result);
                    changed.insert(value.clone());
                }
            }
        }
        // Finish fallible user work (including key cloning/comparison) first.
        let assignment = Arc::new(assignment.clone());
        let changed = changed.into_iter().collect();
        self.assignment = assignment;
        self.positions = Arc::new(next_positions);
        self.shadows = shadows;
        self.pending = Arc::new(BTreeSet::new());
        Ok(changed)
    }
    /// Stage a list move without evaluating shadows. The trial exposes its latest
    /// assignment and pending roots; its changed-shadow list is empty. Dropping
    /// restores the previous assignment, shadows and queue. Commit retains the
    /// pending work for a later flush or an eager `update`/`apply`.
    pub fn queue_move(
        &mut self,
        movement: &ListMove<V>,
        dirty: &[V],
    ) -> Result<ListCascadeMove<'_, V, S>, ListCascadeError<V>> {
        let mut assignment = (*self.assignment).clone();
        self.problem.apply(&mut assignment, movement)?.commit();
        let saved = self.clone();
        self.queue_update(&assignment, dirty)?;
        Ok(ListCascadeMove {
            cache: self,
            saved: Some(saved),
            changed: vec![],
        })
    }
    /// Apply a native list move and propagate its shadows. Dropping the returned
    /// trial restores assignment and shadows; commit retains both atomically.
    pub fn apply(
        &mut self,
        movement: &ListMove<V>,
        dirty: &[V],
        evaluate: impl FnMut(ListCascadeContext<'_, V, S>) -> Option<S>,
    ) -> Result<ListCascadeMove<'_, V, S>, ListCascadeError<V>> {
        let mut assignment = (*self.assignment).clone();
        self.problem.apply(&mut assignment, movement)?.commit();
        let saved = self.clone();
        let changed = self.update(&assignment, dirty, evaluate)?;
        Ok(ListCascadeMove {
            cache: self,
            saved: Some(saved),
            changed,
        })
    }
}

/// A queued or evaluated list/cascade trial. Read through Deref; drop to undo or commit.
pub struct ListCascadeMove<'a, V, S> {
    cache: &'a mut ListCascadeCache<V, S>,
    saved: Option<ListCascadeCache<V, S>>,
    changed: Vec<V>,
}
impl<V, S> ListCascadeMove<'_, V, S> {
    pub fn changed(&self) -> &[V] {
        &self.changed
    }
    pub fn commit(mut self) {
        self.saved = None;
    }
}
impl<V, S> Deref for ListCascadeMove<'_, V, S> {
    type Target = ListCascadeCache<V, S>;
    fn deref(&self) -> &Self::Target {
        self.cache
    }
}
impl<V, S> Drop for ListCascadeMove<'_, V, S> {
    fn drop(&mut self) {
        if let Some(saved) = self.saved.take() {
            *self.cache = saved;
        }
    }
}

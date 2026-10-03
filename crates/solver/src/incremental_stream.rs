//! Persistent, explicitly connected stream operators.
//!
//! Feed every output change into the next node before reading the pipeline's
//! score. Each call commits one input change; an entire graph/batch is not a
//! transaction. Stable IDs distinguish equal-valued occurrences. No-op removals
//! are allowed, and an insertion at an existing ID replaces that occurrence.
//!
//! Callback failures before mutation leave that node unchanged. Some operators
//! poison themselves after ordering/cloning panics during mutation; see each
//! operator's contract. Rebuild poisoned nodes. Changes already delivered to other
//! nodes are not rolled back. Immutable facts and deterministic callbacks remain
//! required. [`crate::StreamGraphBuilder`] wraps these operators with automatic
//! routing and graph transactions; [`crate::StreamScoreDirector`] binds such a
//! graph to finite basic-variable moves.
//!
//! ```
//! use timetuck::{GroupNode, IntSummary, ScoreNode, SimpleScore, TupleChange};
//! let mut loads = GroupNode::<u64, &str>::new();
//! let mut score = ScoreNode::new(|s: &IntSummary| SimpleScore::new(-(s.sum - 10).max(0)));
//! for change in loads.apply(TupleChange::put(1, ("machine", 12))) {
//!     score.apply(change).unwrap();
//! }
//! assert_eq!(score.score(), SimpleScore::new(-2));
//! for change in loads.apply(TupleChange::retract(1)) {
//!     score.apply(change).unwrap();
//! }
//! assert_eq!(score.score(), SimpleScore::ZERO);
//! ```

mod partitioned;
pub use partitioned::PartitionedJoinNode;

use crate::SnapshotMap;
use std::any::Any;
use std::fmt;

use crate::{AdditiveScore, IntCollector, IntSummary};

/// Insert/replace (`Some`) or retract (`None`) one stable tuple identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TupleChange<I, T> {
    pub id: I,
    pub value: Option<T>,
}

impl<I, T> TupleChange<I, T> {
    pub fn put(id: I, value: T) -> Self {
        Self {
            id,
            value: Some(value),
        }
    }

    pub fn retract(id: I) -> Self {
        Self { id, value: None }
    }

    pub fn map<U>(self, mapping: impl FnOnce(T) -> U) -> TupleChange<I, U> {
        TupleChange {
            id: self.id,
            value: self.value.map(mapping),
        }
    }
}

fn healthy(poisoned: bool) {
    assert!(
        !poisoned,
        "incremental stream node is poisoned; rebuild the pipeline"
    );
}

/// Caches the predicate result for each input identity. Retraction never reruns
/// the predicate, and an update evaluates only the changed row.
pub struct FilterNode<'a, I, T> {
    rows: SnapshotMap<I, (bool, T)>,
    predicate: Box<dyn Fn(&T) -> bool + 'a>,
    poisoned: bool,
}

impl<'a, I: Ord + Clone, T: Clone> FilterNode<'a, I, T> {
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.poisoned = false;
    }

    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        T: 'static,
    {
        healthy(self.poisoned);
        Box::new(self.rows.clone())
    }

    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        T: 'static,
    {
        self.rows = *checkpoint
            .downcast::<SnapshotMap<I, (bool, T)>>()
            .expect("filter checkpoint type");
        self.poisoned = false;
    }

    pub fn new(predicate: impl Fn(&T) -> bool + 'a) -> Self {
        Self {
            rows: SnapshotMap::new(),
            predicate: Box::new(predicate),
            poisoned: false,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn rows(&self) -> impl Iterator<Item = (&I, &T)> {
        healthy(self.poisoned);
        self.rows
            .iter()
            .filter_map(|(id, (accepted, row))| accepted.then_some((id, row)))
    }

    pub fn apply(&mut self, change: TupleChange<I, T>) -> Option<TupleChange<I, T>> {
        healthy(self.poisoned);
        let new = change.value.map(|row| ((self.predicate)(&row), row));
        let was_accepted = self
            .rows
            .get(&change.id)
            .is_some_and(|(accepted, _)| *accepted);
        let output = match &new {
            Some((true, row)) => Some(TupleChange::put(change.id.clone(), row.clone())),
            _ if was_accepted => Some(TupleChange::retract(change.id.clone())),
            _ => None,
        };
        self.poisoned = true;
        match new {
            Some(row) => {
                self.rows.insert(change.id, row);
            }
            None => {
                self.rows.remove(&change.id);
            }
        }
        self.poisoned = false;
        output
    }
}

/// Ordered key relation evaluated as `left_key comparison right_key`.
/// Keys use native Rust `Ord`; callers explicitly filter missing keys or select
/// an Option ordering. There is no implicit Java null ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinComparison {
    Equal,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}
impl JoinComparison {
    pub fn matches<K: Ord>(self, left: &K, right: &K) -> bool {
        match self {
            Self::Equal => left == right,
            Self::LessThan => left < right,
            Self::LessThanOrEqual => left <= right,
            Self::GreaterThan => left > right,
            Self::GreaterThanOrEqual => left >= right,
        }
    }
    pub fn reversed(self) -> Self {
        match self {
            Self::Equal => Self::Equal,
            Self::LessThan => Self::GreaterThan,
            Self::LessThanOrEqual => Self::GreaterThanOrEqual,
            Self::GreaterThan => Self::LessThan,
            Self::GreaterThanOrEqual => Self::LessThanOrEqual,
        }
    }
}

#[derive(Clone)]
struct JoinSide<I, T, K> {
    rows: SnapshotMap<I, (K, T)>,
    index: SnapshotMap<K, SnapshotMap<I, ()>>,
    poisoned: bool,
}

impl<I, T, K> JoinSide<I, T, K> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            index: SnapshotMap::new(),
            poisoned: false,
        }
    }
}

type JoinChanges<I, J, T, U> = Vec<TupleChange<(I, J), (T, U)>>;

impl<I: Ord + Clone, T: Clone, K: Ord + Clone> JoinSide<I, T, K> {
    fn matching_ids<'b>(
        &'b self,
        key: &'b K,
        comparison: JoinComparison,
    ) -> impl Iterator<Item = &'b I> {
        use std::ops::Bound::{Excluded, Included, Unbounded};
        let (exact, range) = match comparison {
            JoinComparison::Equal => (self.index.get(key), None),
            JoinComparison::LessThan => (None, Some(self.index.range(Excluded(key), Unbounded))),
            JoinComparison::LessThanOrEqual => {
                (None, Some(self.index.range(Included(key), Unbounded)))
            }
            JoinComparison::GreaterThan => (None, Some(self.index.range(Unbounded, Excluded(key)))),
            JoinComparison::GreaterThanOrEqual => {
                (None, Some(self.index.range(Unbounded, Included(key))))
            }
        };
        exact
            .into_iter()
            .flat_map(|ids| ids.keys())
            .chain(range.into_iter().flatten().flat_map(|(_, ids)| ids.keys()))
    }
    fn apply<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &JoinSide<J, U, K>,
        change: TupleChange<I, T>,
        key: &dyn Fn(&T) -> K,
        comparison: JoinComparison,
    ) -> JoinChanges<I, J, T, U> {
        healthy(self.poisoned || other.poisoned);
        let new = change.value.map(|row| (key(&row), row));
        let same_key = self
            .rows
            .get(&change.id)
            .zip(new.as_ref())
            .is_some_and(|((old, _), (new, _))| old == new);
        let mut output = Vec::new();
        if !same_key && let Some((old_key, _)) = self.rows.get(&change.id) {
            for id in other.matching_ids(old_key, comparison) {
                output.push(TupleChange::retract((change.id.clone(), id.clone())));
            }
        }
        if let Some((new_key, row)) = &new {
            for id in other.matching_ids(new_key, comparison) {
                output.push(TupleChange::put(
                    (change.id.clone(), id.clone()),
                    (
                        row.clone(),
                        other.rows.get(id).expect("indexed row").1.clone(),
                    ),
                ));
            }
        }
        // All projection calls and output row clones happen before mutation.
        self.poisoned = true;
        if same_key {
            // Membership and pair identities are unchanged. Downstream nodes
            // retract their captured old payload when receiving the replacement.
            self.rows
                .insert(change.id, new.expect("same-key replacement"));
            self.poisoned = false;
            return output;
        }
        if let Some((old_key, _)) = self.rows.get(&change.id) {
            let mut ids = self.index.get(old_key).expect("indexed key").clone();
            ids.remove(&change.id);
            if ids.is_empty() {
                self.index.remove(old_key);
            } else {
                self.index.insert(old_key.clone(), ids);
            }
        }
        self.rows.remove(&change.id);
        if let Some((new_key, row)) = new {
            let mut ids = self.index.get(&new_key).cloned().unwrap_or_default();
            ids.insert(change.id.clone(), ());
            self.index.insert(new_key.clone(), ids);
            self.rows.insert(change.id, (new_key, row));
        }
        self.poisoned = false;
        output
    }
}

/// Persistent equality or ordered comparison join. Cached keys make retraction
/// independent of callbacks. Range traversal skips nonmatching key buckets.
/// Cartesian joins use a constant key. Output identity is `(left_id, right_id)`.
/// Same-key updates replace surviving pairs once and retain index membership.
/// Key changes retract old matches before inserting new matches. Feed both sides
/// for self-joins, in either order, before reading
/// the final score. Each side indexes all rows, even currently unmatched ones.
pub struct JoinNode<'a, I, J, L, R, K> {
    comparison: JoinComparison,
    left: JoinSide<I, L, K>,
    right: JoinSide<J, R, K>,
    left_key: Box<dyn Fn(&L) -> K + 'a>,
    right_key: Box<dyn Fn(&R) -> K + 'a>,
}

impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    JoinNode<'a, I, J, L, R, K>
{
    pub(crate) fn clear(&mut self) {
        self.left = JoinSide::new();
        self.right = JoinSide::new();
    }

    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
    {
        healthy(self.is_poisoned());
        Box::new((self.left.clone(), self.right.clone()))
    }

    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
    {
        (self.left, self.right) = *checkpoint
            .downcast::<(JoinSide<I, L, K>, JoinSide<J, R, K>)>()
            .expect("join checkpoint type");
    }

    pub fn new(left_key: impl Fn(&L) -> K + 'a, right_key: impl Fn(&R) -> K + 'a) -> Self {
        Self::comparing(JoinComparison::Equal, left_key, right_key)
    }

    /// Build an indexed ordered comparison join. All input keys are captured.
    pub fn comparing(
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Self {
        Self {
            comparison,
            left: JoinSide::new(),
            right: JoinSide::new(),
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.left.poisoned || self.right.poisoned
    }

    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> JoinChanges<I, J, L, R> {
        self.left
            .apply(&self.right, change, &self.left_key, self.comparison)
    }

    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> JoinChanges<I, J, L, R> {
        self.right
            .apply(
                &self.left,
                change,
                &self.right_key,
                self.comparison.reversed(),
            )
            .into_iter()
            .map(|change| TupleChange {
                id: (change.id.1, change.id.0),
                value: change.value.map(|(right, left)| (left, right)),
            })
            .collect()
    }

    /// Inspection walks the current join, ordered by left ID, right key, then
    /// right ID. For equality joins this is left ID then right ID.
    /// Updates themselves visit only old/new matching key buckets.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        healthy(self.is_poisoned());
        self.left
            .rows
            .iter()
            .flat_map(move |(left_id, (key, left))| {
                self.right
                    .matching_ids(key, self.comparison)
                    .map(move |right_id| {
                        (
                            (left_id, right_id),
                            (left, &self.right.rows.get(right_id).expect("indexed row").1),
                        )
                    })
            })
    }
}

/// Incrementally aggregates mapped integer occurrences, keyed by stable input ID.
/// Moving an input between keys touches only the old/new groups. An empty group
/// emits a retraction; a replacement within the same key emits one final summary.
pub struct GroupNode<I, K> {
    inputs: SnapshotMap<I, (K, i64)>,
    groups: SnapshotMap<K, IntCollector>,
    poisoned: bool,
}

impl<I: Ord, K: Ord + Clone> Default for GroupNode<I, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Ord, K: Ord + Clone> GroupNode<I, K> {
    pub(crate) fn clear(&mut self) {
        self.inputs.clear();
        self.groups.clear();
        self.poisoned = false;
    }

    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: Clone + 'static,
        K: 'static,
    {
        healthy(self.poisoned);
        Box::new((self.inputs.clone(), self.groups.clone()))
    }

    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
    {
        (self.inputs, self.groups) = *checkpoint
            .downcast::<(SnapshotMap<I, (K, i64)>, SnapshotMap<K, IntCollector>)>()
            .expect("group checkpoint type");
        self.poisoned = false;
    }

    pub fn new() -> Self {
        Self {
            inputs: SnapshotMap::new(),
            groups: SnapshotMap::new(),
            poisoned: false,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn groups(&self) -> impl Iterator<Item = (&K, IntSummary)> {
        healthy(self.poisoned);
        self.groups
            .iter()
            .map(|(key, collector)| (key, collector.summary()))
    }

    pub fn apply(&mut self, change: TupleChange<I, (K, i64)>) -> Vec<TupleChange<K, IntSummary>> {
        healthy(self.poisoned);
        let old_key = self.inputs.get(&change.id).map(|(key, _)| key.clone());
        let new_key = change.value.as_ref().map(|(key, _)| key.clone());
        self.poisoned = true;
        if let Some((key, value)) = self.inputs.get(&change.id) {
            let mut collector = self.groups.get(key).expect("input group").clone();
            assert!(collector.remove(*value), "input occurrence");
            if collector.summary().count == 0 {
                self.groups.remove(key);
            } else {
                self.groups.insert(key.clone(), collector);
            }
        }
        self.inputs.remove(&change.id);
        if let Some((key, value)) = change.value {
            let mut collector = self.groups.get(&key).cloned().unwrap_or_default();
            collector.insert(value);
            self.groups.insert(key.clone(), collector);
            self.inputs.insert(change.id, (key, value));
        }
        let mut output = Vec::new();
        if let Some(key) = old_key.as_ref() {
            output.push(TupleChange {
                id: key.clone(),
                value: self.groups.get(key).map(IntCollector::summary),
            });
        }
        if let Some(key) = new_key
            && old_key.as_ref() != Some(&key)
        {
            let value = Some(self.groups.get(&key).expect("new group").summary());
            output.push(TupleChange { id: key, value });
        }
        self.poisoned = false;
        output
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructuralStreamContribution;

impl fmt::Display for StructuralStreamContribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("incremental stream contribution has nonzero structural state")
    }
}
impl std::error::Error for StructuralStreamContribution {}

/// A terminal with one cached contribution per input identity. Retractions use
/// the cached score, with no callback replay. Structural contributions are errors.
pub struct ScoreNode<'a, I, T, S> {
    contributions: SnapshotMap<I, S>,
    evaluate: Box<dyn Fn(&T) -> S + 'a>,
    score: S,
    poisoned: bool,
}

impl<'a, I: Ord, T, S: AdditiveScore> ScoreNode<'a, I, T, S> {
    pub fn new(evaluate: impl Fn(&T) -> S + 'a) -> Self {
        Self {
            contributions: SnapshotMap::new(),
            evaluate: Box::new(evaluate),
            score: S::ZERO,
            poisoned: false,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn score(&self) -> S {
        healthy(self.poisoned);
        self.score
    }

    pub fn contributions(&self) -> impl Iterator<Item = (&I, S)> {
        healthy(self.poisoned);
        self.contributions.iter().map(|(id, &score)| (id, score))
    }

    pub fn apply(&mut self, change: TupleChange<I, T>) -> Result<(), StructuralStreamContribution> {
        healthy(self.poisoned);
        let new = change.value.as_ref().map(|row| (self.evaluate)(row));
        if new.is_some_and(|score| score.structural() != 0) {
            return Err(StructuralStreamContribution);
        }
        let score = self.score
            - self
                .contributions
                .get(&change.id)
                .copied()
                .unwrap_or(S::ZERO)
            + new.unwrap_or(S::ZERO);
        self.poisoned = true;
        if let Some(new) = new {
            self.contributions.insert(change.id, new);
        } else {
            self.contributions.remove(&change.id);
        }
        self.score = score;
        self.poisoned = false;
        Ok(())
    }
}

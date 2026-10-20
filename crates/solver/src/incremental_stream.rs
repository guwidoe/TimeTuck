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

mod filter_bound;
mod filter_pairs;
mod group_batch;
mod group_entry;
#[cfg(test)]
mod group_immediate_tests;
mod group_inputs;
#[cfg(test)]
mod group_primitive_tests;
#[cfg(test)]
mod rejected_cache_tests;
mod sum_group;
pub use sum_group::SumGroupNode;
mod join_batch;
mod partitioned;
pub use partitioned::{
    JoinPartitionReservation, JoinPartitionSide, JoinPartitionState, PartitionedJoinNode,
};

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
    rows: crate::primitive_rows::PrimitiveRows<I, (bool, T)>,
    fixed_rows: Option<Box<join_fixed_rows::FixedRows<I, (bool, T)>>>,
    pair_rows: Option<Box<filter_pairs::PairRows<I, (bool, T)>>>,
    predicate: Box<dyn Fn(&T) -> bool + 'a>,
    // Only an initially empty, primitive-ID, NoDrop cache may omit rejected
    // inputs. Public standalone nodes retain their original ownership behavior.
    elide_rejected: bool,
    // Once an unbounded rejection was discarded, retained rows cannot prove
    // a smaller universe. This conservative capability only ever turns off.
    rejected_universe_incomplete: bool,
    poisoned: bool,
}

impl<'a, I: Ord + Clone, T: Clone> FilterNode<'a, I, T> {
    pub(crate) fn clear(&mut self) {
        self.clear_rows();
        self.poisoned = false;
    }

    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        T: 'static,
    {
        healthy(self.poisoned);
        self.checkpoint_rows()
    }

    pub(crate) fn begin_checkpoint(&mut self) -> Box<dyn Any>
    where
        I: 'static,
        T: 'static,
    {
        healthy(self.poisoned);
        self.begin_rows()
    }
    pub(crate) fn commit_checkpoint(&mut self, saved: Box<dyn Any>)
    where
        I: 'static,
        T: 'static,
    {
        self.commit_rows(saved);
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        T: 'static,
    {
        self.restore_rows(checkpoint);
        self.poisoned = false;
    }

    pub fn new(predicate: impl Fn(&T) -> bool + 'a) -> Self {
        Self {
            rows: crate::primitive_rows::PrimitiveRows::new(),
            fixed_rows: None,
            pair_rows: None,
            predicate: Box::new(predicate),
            elide_rejected: false,
            rejected_universe_incomplete: false,
            poisoned: false,
        }
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn rows(&self) -> impl Iterator<Item = (&I, &T)> {
        healthy(self.poisoned);
        self.row_iter()
            .filter_map(|(id, (accepted, row))| accepted.then_some((id, row)))
    }

    pub fn apply(&mut self, change: TupleChange<I, T>) -> Option<TupleChange<I, T>> {
        healthy(self.poisoned);
        let new = change.value.map(|row| ((self.predicate)(&row), row));
        self.apply_filtered(change.id, new)
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
    rows: crate::primitive_rows::PrimitiveRows<I, (K, T)>,
    fixed_rows: Option<Box<join_fixed_rows::FixedRows<I, (K, T)>>>,
    fixed_index: Option<Box<join_fixed_index::FixedIndex<I, K>>>,
    index: SnapshotMap<K, SnapshotMap<I, ()>>,
    poisoned: bool,
}

impl<I, T, K> JoinSide<I, T, K> {
    fn new() -> Self {
        Self {
            rows: crate::primitive_rows::PrimitiveRows::new(),
            fixed_rows: None,
            fixed_index: None,
            index: SnapshotMap::new(),
            poisoned: false,
        }
    }
}

type JoinChanges<I, J, T, U> = Vec<TupleChange<(I, J), (T, U)>>;

impl<I: Ord + Clone, T: Clone, K: Ord + Clone> JoinSide<I, T, K> {
    fn row(&self, id: &I) -> Option<&(K, T)> {
        match &self.fixed_rows {
            Some(rows) => rows.get(id),
            None => self.rows.get(id),
        }
    }
    fn row_iter(&self) -> impl Iterator<Item = (&I, &(K, T))> {
        self.fixed_rows
            .iter()
            .flat_map(|rows| rows.iter())
            .chain(self.rows.iter())
    }
    fn insert_row(&mut self, id: I, value: (K, T)) {
        match &mut self.fixed_rows {
            Some(rows) => rows.insert(id, value),
            None => self.rows.insert(id, value),
        }
    }
    fn remove_row(&mut self, id: &I) {
        match &mut self.fixed_rows {
            Some(rows) => rows.remove(id),
            None => {
                self.rows.remove(id);
            }
        }
    }
    fn clear_rows(&mut self) {
        match &mut self.fixed_rows {
            Some(rows) => rows.clear(),
            None => self.rows.clear(),
        }
    }
    fn matching_ids<'b>(
        &'b self,
        key: &'b K,
        comparison: JoinComparison,
    ) -> impl Iterator<Item = &'b I> {
        self.matching_ids_sized(key, comparison).1
    }
    fn matching_ids_sized<'b>(
        &'b self,
        key: &'b K,
        comparison: JoinComparison,
    ) -> (usize, impl Iterator<Item = &'b I>) {
        use std::ops::Bound::{Excluded, Included, Unbounded};
        let fixed = self
            .fixed_index
            .as_ref()
            .map(|index| index.matching(key, comparison));
        let (exact, range) = if fixed.is_some() {
            (None, None)
        } else {
            match comparison {
                JoinComparison::Equal => (self.index.get(key), None),
                JoinComparison::LessThan => {
                    (None, Some(self.index.range(Excluded(key), Unbounded)))
                }
                JoinComparison::LessThanOrEqual => {
                    (None, Some(self.index.range(Included(key), Unbounded)))
                }
                JoinComparison::GreaterThan => {
                    (None, Some(self.index.range(Unbounded, Excluded(key))))
                }
                JoinComparison::GreaterThanOrEqual => {
                    (None, Some(self.index.range(Unbounded, Included(key))))
                }
            }
        };
        let count = fixed
            .as_ref()
            .map_or_else(|| exact.map_or(0, SnapshotMap::len), |(count, _)| *count);
        (
            count,
            fixed.into_iter().flat_map(|(_, ids)| ids).chain(
                exact
                    .into_iter()
                    .flat_map(|ids| ids.keys())
                    .chain(range.into_iter().flatten().flat_map(|(_, ids)| ids.keys())),
            ),
        )
    }
    fn apply<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &JoinSide<J, U, K>,
        change: TupleChange<I, T>,
        key: &dyn Fn(&T) -> K,
        comparison: JoinComparison,
    ) -> JoinChanges<I, J, T, U> {
        self.apply_buffered(other, change, key, comparison, Vec::new())
    }
    fn apply_buffered<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &JoinSide<J, U, K>,
        change: TupleChange<I, T>,
        key: &dyn Fn(&T) -> K,
        comparison: JoinComparison,
        output_buffer: JoinChanges<I, J, T, U>,
    ) -> JoinChanges<I, J, T, U> {
        healthy(self.poisoned || other.poisoned);
        let new = change.value.map(|row| (key(&row), row));
        let same_key = self
            .row(&change.id)
            .zip(new.as_ref())
            .is_some_and(|((old, _), (new, _))| old == new);
        // Own the buffer locally before cloning outputs. Unwinding drops every
        // partially prepared row at the same point as the standalone path.
        let mut output = output_buffer;
        debug_assert!(output.is_empty());
        if !same_key && let Some((old_key, _)) = self.row(&change.id) {
            let (count, ids) = other.matching_ids_sized(old_key, comparison);
            output.reserve(count);
            for id in ids {
                output.push(TupleChange::retract((change.id.clone(), id.clone())));
            }
        }
        if let Some((new_key, row)) = &new {
            let (count, ids) = other.matching_ids_sized(new_key, comparison);
            output.reserve(count);
            for id in ids {
                output.push(TupleChange::put(
                    (change.id.clone(), id.clone()),
                    (row.clone(), other.row(id).expect("indexed row").1.clone()),
                ));
            }
        }
        // All projection calls and output row clones happen before mutation.
        self.install_projected(change.id, new, same_key);
        output
    }
    fn install_projected(&mut self, id: I, new: Option<(K, T)>, same_key: bool) {
        self.poisoned = true;
        if same_key {
            // Membership and pair identities are unchanged. Downstream nodes
            // retract their captured old payload when receiving the replacement.
            self.insert_row(id, new.expect("same-key replacement"));
            self.poisoned = false;
            return;
        }
        // Borrow row storage separately from the mutable membership index.
        let old = match &self.fixed_rows {
            Some(rows) => rows.get(&id),
            None => self.rows.get(&id),
        };
        if let Some((old_key, _)) = old {
            if let Some(index) = &mut self.fixed_index {
                index.remove(old_key, &id);
            } else {
                let mut ids = self.index.get(old_key).expect("indexed key").clone();
                ids.remove(&id);
                if ids.is_empty() {
                    self.index.remove(old_key);
                } else {
                    self.index.insert(old_key.clone(), ids);
                }
            }
        }
        // Retain the old captured payload's destruction point, before any
        // new bucket key is cloned, including bounded row storage.
        self.remove_row(&id);
        if let Some((new_key, row)) = new {
            if let Some(index) = &mut self.fixed_index {
                index.insert(&new_key, &id);
            } else {
                let mut ids = self.index.get(&new_key).cloned().unwrap_or_default();
                ids.insert(id.clone(), ());
                self.index.insert(new_key.clone(), ids);
            }
            self.insert_row(id, (new_key, row));
        }
        self.poisoned = false;
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
    // Only ordinary graph construction enables journals for unbounded joins.
    primitive_graph_journals: bool,
}

impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    JoinNode<'a, I, J, L, R, K>
{
    pub(crate) fn clear(&mut self) {
        self.left.clear_rows();
        self.left.index.clear();
        if let Some(index) = &mut self.left.fixed_index {
            index.clear();
        }
        self.left.poisoned = false;
        self.right.clear_rows();
        self.right.index.clear();
        if let Some(index) = &mut self.right.fixed_index {
            index.clear();
        }
        self.right.poisoned = false;
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
        let (left, right) = *checkpoint
            .downcast::<(JoinSide<I, L, K>, JoinSide<J, R, K>)>()
            .expect("join checkpoint type");
        self.left.restore_side(left);
        self.right.restore_side(right);
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
            primitive_graph_journals: false,
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

    // Graph-only scratch storage. Right outputs deliberately retain their raw
    // orientation until all clones and cache mutations have completed.
    pub(crate) fn apply_left_buffered(
        &mut self,
        change: TupleChange<I, L>,
        output: JoinChanges<I, J, L, R>,
    ) -> JoinChanges<I, J, L, R> {
        self.left
            .apply_buffered(&self.right, change, &self.left_key, self.comparison, output)
    }

    pub(crate) fn apply_right_buffered(
        &mut self,
        change: TupleChange<J, R>,
        output: JoinChanges<J, I, R, L>,
    ) -> JoinChanges<J, I, R, L> {
        self.right.apply_buffered(
            &self.left,
            change,
            &self.right_key,
            self.comparison.reversed(),
            output,
        )
    }

    /// Inspection walks the current join, ordered by left ID, right key, then
    /// right ID. For equality joins this is left ID then right ID.
    /// Updates themselves visit only old/new matching key buckets.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        healthy(self.is_poisoned());
        self.left
            .row_iter()
            .flat_map(move |(left_id, (key, left))| {
                self.right
                    .matching_ids(key, self.comparison)
                    .map(move |right_id| {
                        (
                            (left_id, right_id),
                            (left, &self.right.row(right_id).expect("indexed row").1),
                        )
                    })
            })
    }
}

/// Incrementally aggregates mapped integer occurrences, keyed by stable input ID.
/// Moving an input between keys touches only the old/new groups. An empty group
/// emits a retraction; a replacement within the same key emits one final summary.
pub struct GroupNode<I, K> {
    inputs: group_inputs::GroupInputs<I, (K, i64)>,
    groups: crate::primitive_rows::PrimitiveRows<K, IntCollector>,
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
        let (inputs, groups) = *checkpoint
            .downcast::<(
                group_inputs::GroupInputs<I, (K, i64)>,
                crate::primitive_rows::PrimitiveRows<K, IntCollector>,
            )>()
            .expect("group checkpoint type");
        self.inputs.install_snapshot(inputs);
        self.groups.install_snapshot(groups);
        self.poisoned = false;
    }

    pub fn new() -> Self {
        Self {
            inputs: group_inputs::GroupInputs::new(),
            groups: crate::primitive_rows::PrimitiveRows::new(),
            poisoned: false,
        }
    }

    // Group mappings choose their own key coordinates. Primitive key storage
    // needs only exact type identity, independently of source occurrence bounds.
    pub(crate) fn bind_primitive_group_keys(&mut self) -> bool
    where
        K: 'static,
    {
        let bound = self.groups.bind_primitive();
        if bound {
            self.groups.enable_int_collector_pool();
        }
        bound
    }

    // Expanded occurrences have their own ID coordinates. Exact primitive ID
    // storage remains valid without a declared Cartesian or ordinal universe.
    pub(crate) fn bind_primitive_inputs(&mut self) -> bool
    where
        I: 'static,
    {
        if std::mem::needs_drop::<K>() {
            return false;
        }
        self.inputs.bind_primitive()
    }

    pub(crate) fn bind_pair_inputs(&mut self, left: usize, right: usize) -> bool
    where
        I: 'static,
    {
        self.inputs.bind_pair(left, right)
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
        self.apply_inline(change).into_iter().flatten().collect()
    }

    pub(crate) fn apply_inline(
        &mut self,
        change: TupleChange<I, (K, i64)>,
    ) -> [Option<TupleChange<K, IntSummary>>; 2] {
        healthy(self.poisoned);
        let (input, old) = self.inputs.prepare_change(&change.id, |old| {
            old.map(|(key, value)| (key.clone(), *value))
        });
        let old_key = old.as_ref().map(|(key, _)| key);
        let new_key = change.value.as_ref().map(|(key, _)| key.clone());
        self.poisoned = true;
        let same_group = old_key.is_some() && old_key == new_key.as_ref();
        let summaries = if self.groups.is_primitive() {
            Some(group_entry::apply(
                &mut self.groups,
                old.as_ref(),
                change.value.as_ref(),
                new_key.as_ref(),
                same_group,
            ))
        } else {
            if same_group {
                let (key, old) = old.as_ref().expect("existing input");
                let new = change.value.as_ref().expect("replacement input").1;
                let mut collector = self.groups.get(key).expect("input group").clone();
                // A sole capture empties its group before reinsertion. Retain that
                // public key-representative behavior; other replacements keep the
                // existing group and need only one value update in each map.
                if collector.summary().count == 1 {
                    assert!(collector.remove(*old), "input occurrence");
                    self.groups.remove(key);
                    collector.insert(new);
                    self.groups
                        .insert(new_key.as_ref().unwrap().clone(), collector);
                } else if *old != new {
                    assert!(collector.replace_captured(*old, new), "input occurrence");
                    self.groups.insert(key.clone(), collector);
                }
            } else {
                if let Some((key, value)) = old.as_ref() {
                    let mut collector = self.groups.get(key).expect("input group").clone();
                    assert!(collector.remove(*value), "input occurrence");
                    if collector.summary().count == 0 {
                        self.groups.remove(key);
                    } else {
                        self.groups.insert(key.clone(), collector);
                    }
                }
                if let Some((key, value)) = change.value.as_ref() {
                    let mut collector = self.groups.get(key).cloned().unwrap_or_default();
                    collector.insert(*value);
                    self.groups.insert(key.clone(), collector);
                }
            }
            None
        };
        // Preparation locates the captured input only once. An existing ID is
        // a value replacement, preserving journaled paths and exclusive roots.
        if let Some(value) = change.value {
            input.insert(change.id, value);
        } else {
            input.remove();
        }
        let old_output = old_key.map(|key| TupleChange {
            id: key.clone(),
            value: match summaries {
                Some((old, _)) => old,
                None => self.groups.get(key).map(IntCollector::summary),
            },
        });
        let new_output = if let Some(key) = new_key
            && !same_group
        {
            let value = Some(match summaries {
                Some((_, new)) => new.expect("new group"),
                None => self.groups.get(&key).expect("new group").summary(),
            });
            Some(TupleChange { id: key, value })
        } else {
            None
        };
        self.poisoned = false;
        [old_output, new_output]
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
        self.score.clone()
    }

    pub fn contributions(&self) -> impl Iterator<Item = (&I, S)> {
        healthy(self.poisoned);
        self.contributions
            .iter()
            .map(|(id, score)| (id, score.clone()))
    }

    pub fn apply(&mut self, change: TupleChange<I, T>) -> Result<(), StructuralStreamContribution> {
        healthy(self.poisoned);
        let new = change.value.as_ref().map(|row| (self.evaluate)(row));
        if new.as_ref().is_some_and(|score| score.structural() != 0) {
            return Err(StructuralStreamContribution);
        }
        let score = self.score.clone()
            - self
                .contributions
                .get(&change.id)
                .cloned()
                .unwrap_or(S::ZERO)
            + new.clone().unwrap_or(S::ZERO);
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

pub mod batched_join;

mod join_bound;
mod join_fixed_index;
mod join_fixed_rows;
mod join_journal;

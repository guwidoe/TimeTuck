//! Typed acyclic stream graphs with automatic event routing and transactional state.
//!
//! Topology is fixed when the builder is consumed. A transaction checkpoints each
//! visited node once, including its cached indexes, then applies incremental tuple
//! changes. Dropping it restores those checkpoints without replaying callbacks.
//! Checkpoints share persistent tree roots; updates copy only affected tree paths.
//! Facts and keys must have stable identity/ordering and no interior
//! mutation. Ordinary callback unwinding rolls back; abort, forgotten guards and
//! panicking destructors have the usual Rust transaction limitations.
//!
//! ```
//! use timetuck::{SimpleScore, StreamGraphBuilder, TupleChange};
//! let mut builder = StreamGraphBuilder::<SimpleScore>::new();
//! let jobs = builder.source::<usize, (u32, i64)>();
//! let loads = builder.group_collect(jobs.port(), |&(machine, weight)| (machine, weight))?;
//! let capacity = builder.score("capacity", &loads,
//!     |(_, summary)| SimpleScore::new(-(summary.sum - 10).max(0)))?;
//! let mut graph = builder.build();
//! {
//!     let mut trial = graph.transaction();
//!     trial.apply(&jobs, TupleChange::put(0, (1, 12)))?;
//!     assert_eq!(trial.score()?, SimpleScore::new(-2));
//! } // Roll back without evaluating any callbacks again.
//! assert_eq!(graph.score(), SimpleScore::ZERO);
//! let mut accepted = graph.transaction();
//! accepted.apply(&jobs, TupleChange::put(0, (1, 12)))?;
//! accepted.commit()?;
//! assert_eq!(graph.constraint_matches(&capacity)?[&1].1, SimpleScore::new(-2));
//! # Ok::<(), timetuck::GraphError>(())
//! ```

mod dependency_batch;
mod dynamic_dependency;
mod facts;
pub use facts::FactTransaction;
mod activity;
mod indexed_batch;
mod ordered_existence;
mod partitioned_batch;
mod stable_flat_map;
use activity::ActivityRule;
pub use activity::{NodeActivity, StreamActivity};
mod dataset;
mod precompute;
pub use precompute::SharedPrecompute;
mod shadows;
mod shared_sets;
pub use shared_sets::{SharedConcat, SharedDistinct, SharedPaddedConcat};
mod shared_aggregation;
pub use shared_aggregation::{
    SharedCollect, SharedCollectList, SharedGroup, SharedGroupKeys, SharedGroupList,
};
mod compound_join;
pub use compound_join::{JoinCondition, JoinConditions, JoinProjection};
mod shared_relations;
pub use shared_relations::{SharedExistence, SharedJoin};
mod shared;
pub use shared::{SharedFilter, SharedFlatMap, SharedMap};
mod unique_pairs;
pub use dataset::StreamDataset;

use crate::{
    AdditiveScore, CollectNode, DistinctNode, ExistenceMode, ExistsEqualNode, ExistsNode,
    FilterNode, FlatMapNode, GroupNode, IntSummary, JoinComparison, JoinNode, ListCollector,
    ListSnapshot, OverlapJoinNode, PartitionedJoinNode, ReferenceNode, ReversibleCollector,
    SnapshotMap, TupleChange,
};
use std::{
    any::Any,
    collections::{BTreeMap, VecDeque},
    fmt,
    marker::PhantomData,
    rc::Rc,
};

type Event = Rc<dyn Any>;
type Outputs = Vec<Event>;
type GroupMapping<'a, T, K> = dyn Fn(&T) -> (K, i64) + 'a;
pub type JoinedStreamPort<I, J, L, R> = StreamPort<(I, J), (L, R)>;
/// A keyed list result, with its group key in both identity and payload.
pub type GroupedListStreamPort<K, I, V> = StreamPort<K, (K, ListSnapshot<I, V>)>;

/// Identity of an occurrence in a concatenated multiset. Left and right IDs never
/// collide, even if both inputs are the same port. Ordering puts the left branch
/// first, then uses the original ID ordering within each branch. Payload equality
/// does not affect identity; apply `distinct` explicitly for set semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConcatId<L, R> {
    Left(L),
    Right(R),
}
/// Concatenated inputs share a payload type but can have different ID types.
pub type ConcatenatedStreamPort<L, R, T> = StreamPort<ConcatId<L, R>, T>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    ForeignPort,
    ProtectedPlanningSource,
    DuplicateConstraint(String),
    UnknownConstraintWeights {
        unknown: Vec<String>,
        known: Vec<String>,
    },
    UnweightedConstraintOverride(String),
    StructuralConstraintWeight(String),
    ScoreOverflow {
        constraint: Option<String>,
    },
    DecimalWeightArithmetic {
        constraint: String,
        error: crate::DecimalArithmeticError,
    },
    StructuralContribution(String),
    FailedTransaction,
    Dependency(String),
    CorruptedScore(Box<crate::StreamScoreCorruption>),
}
impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownConstraintWeights { unknown, known } => write!(
                f,
                "unknown constraint weights {unknown:?}; known constraints {known:?}"
            ),
            Self::UnweightedConstraintOverride(name) => {
                write!(f, "constraint {name:?} has no configurable weight")
            }
            Self::ScoreOverflow { constraint } => write!(
                f,
                "score overflow in {}",
                constraint.as_deref().unwrap_or("graph total")
            ),
            Self::DecimalWeightArithmetic { constraint, error } => write!(
                f,
                "constraint {constraint:?} decimal impact failed: {error}"
            ),
            Self::StructuralConstraintWeight(name) => {
                write!(f, "constraint {name:?} has a structural weight component")
            }
            Self::Dependency(reason) => write!(f, "invalid derived dependency: {reason}"),
            Self::ProtectedPlanningSource => {
                f.write_str("planning sources cannot be changed by a fact transaction")
            }
            Self::ForeignPort => f.write_str("stream port belongs to another graph"),
            Self::DuplicateConstraint(name) => write!(f, "duplicate stream constraint: {name}"),
            Self::StructuralContribution(name) => {
                write!(f, "stream constraint {name} has a structural contribution")
            }
            Self::CorruptedScore(report) => report.fmt(f),
            Self::FailedTransaction => {
                f.write_str("stream transaction failed; drop it to roll back")
            }
        }
    }
}
impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DecimalWeightArithmetic { error, .. } => Some(error),
            Self::ScoreOverflow { .. } => Some(&crate::ScoreOverflow),
            Self::CorruptedScore(report) => Some(report.as_ref()),
            _ => None,
        }
    }
}

/// A typed output port, tied to one builder/graph. Reusing it explicitly shares
/// its node; closure equivalence is not used to discover implicit sharing.
pub struct StreamPort<I, T> {
    owner: Rc<()>,
    node: usize,
    marker: PhantomData<fn() -> (I, T)>,
}
impl<I, T> Clone for StreamPort<I, T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            node: self.node,
            marker: PhantomData,
        }
    }
}

impl<I, T> StreamPort<I, T> {
    /// Compare graph-node identity even when the typed payloads differ. This
    /// does not compare current rows or equate independent equivalent pipelines.
    pub fn same_node<J, U>(&self, other: &StreamPort<J, U>) -> bool {
        Rc::ptr_eq(&self.owner, &other.owner) && self.node == other.node
    }
}

/// Only source handles permit external tuple updates.
pub struct StreamSource<I, T>(StreamPort<I, T>);
impl<I, T> Clone for StreamSource<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<I, T> StreamSource<I, T> {
    pub(crate) fn node_id(&self) -> usize {
        self.0.node
    }
    pub fn port(&self) -> &StreamPort<I, T> {
        &self.0
    }
}

/// A typed handle for inspecting one constraint's current match tuples/scores.
pub struct StreamConstraint<I, T>(StreamPort<I, T>);
impl<I, T> Clone for StreamConstraint<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone, Copy)]
struct NodeArithmetic {
    impact: crate::ScoringArithmetic,
    totals: crate::ScoringArithmetic,
}
trait Node<S: AdditiveScore> {
    fn prefers_batch(&self) -> bool {
        false
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let mut out = Vec::new();
        for (side, event) in inputs {
            out.extend(self.apply(*side, event, arithmetic)?);
        }
        Ok(out)
    }
    fn constraint_weight(&self) -> Option<&S> {
        None
    }
    fn replace_weight(&mut self, _weight: S) {
        unreachable!("not a weighted constraint")
    }

    fn checkpoint(&self) -> Box<dyn Any>;
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.checkpoint()
    }
    fn commit_checkpoint(&mut self, _checkpoint: Box<dyn Any>) {}
    fn clear(&mut self);
    fn source_count(&self) -> Option<usize> {
        None
    }
    fn source_events(&self) -> Vec<Event> {
        Vec::new()
    }
    fn evaluates_score(&self, _: &Event) -> bool {
        false
    }
    fn scores_match(&self, _: &dyn Any) -> bool {
        true
    }
    fn restore(&mut self, checkpoint: Box<dyn Any>);
    fn apply(
        &mut self,
        side: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError>;
    fn score(&self) -> S {
        S::ZERO
    }
    fn dataset_rows(&self) -> Option<&dyn Any> {
        None
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        None
    }
    fn is_constraint(&self) -> bool {
        false
    }
    fn matches(&self) -> Option<&dyn Any> {
        None
    }
}
fn typed<I: 'static, T: 'static>(event: &Event) -> &TupleChange<I, T> {
    event.downcast_ref().expect("builder enforces event types")
}
fn erase<I: 'static, T: 'static>(changes: impl IntoIterator<Item = TupleChange<I, T>>) -> Outputs {
    changes
        .into_iter()
        .map(|change| Rc::new(change) as Event)
        .collect()
}

struct SourceNode<I, T>(SnapshotMap<I, T>);
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore> Node<S> for SourceNode<I, T> {
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let mut latest = BTreeMap::new();
        for (_, event) in inputs {
            latest.insert(typed::<I, T>(event).id.clone(), event.clone());
        }
        let mut out = Vec::new();
        for event in latest.into_values() {
            out.extend(<Self as Node<S>>::apply(self, false, &event, arithmetic)?);
        }
        Ok(out)
    }

    fn source_count(&self) -> Option<usize> {
        Some(self.0.len())
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    fn source_events(&self) -> Vec<Event> {
        erase(
            self.0
                .iter()
                .map(|(id, value)| TupleChange::put(id.clone(), value.clone())),
        )
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.0.clone())
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.0.begin_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.0.commit_checkpoint(saved);
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore_checkpoint(c);
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        Some(&self.0)
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        if let Some(value) = &change.value {
            self.0.insert(change.id.clone(), value.clone());
        } else {
            self.0.remove(&change.id);
        }
        Ok(vec![event.clone()])
    }
}

struct Filter<'a, I, T>(FilterNode<'a, I, T>);
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore> Node<S> for Filter<'_, I, T> {
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.0.begin_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.0.commit_checkpoint(saved);
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
    }
}

struct Derived<'a, K, T, V>(crate::DependencyNode<'a, K, T, V>);
impl<
    K: Ord + Clone + fmt::Debug + 'static,
    T: Clone + 'static,
    V: Clone + Eq + 'static,
    S: AdditiveScore,
> Node<S> for Derived<'_, K, T, V>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        self.0
            .apply(typed::<K, T>(event).clone())
            .map(erase)
            .map_err(|e| GraphError::Dependency(e.to_string()))
    }
}

struct Map<'a, I, T, U> {
    mapping: Box<dyn Fn(&T) -> U + 'a>,
    marker: PhantomData<fn() -> I>,
}
impl<I: Clone + 'static, T: 'static, U: 'static, S: AdditiveScore> Node<S> for Map<'_, I, T, U> {
    fn clear(&mut self) {}
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn restore(&mut self, _: Box<dyn Any>) {}
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        Ok(erase([TupleChange {
            id: change.id.clone(),
            value: change.value.as_ref().map(|v| (self.mapping)(v)),
        }]))
    }
}

type CachedMapping<'a, C, T, U> = dyn Fn(&mut C, &T) -> U + 'a;
struct CachedMap<'a, I, C, T, U> {
    initial: C,
    states: SnapshotMap<I, C>,
    mapping: Box<CachedMapping<'a, C, T, U>>,
}
impl<I: Ord + Clone + 'static, C: Clone + 'static, T: 'static, U: 'static, S: AdditiveScore> Node<S>
    for CachedMap<'_, I, C, T, U>
{
    fn clear(&mut self) {
        self.states.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.states.clone())
    }
    fn restore(&mut self, checkpoint: Box<dyn Any>) {
        self.states = *checkpoint.downcast().expect("cached map checkpoint type");
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let mut states = self.states.clone();
        let output = if let Some(value) = &change.value {
            let mut state = states.get(&change.id).unwrap_or(&self.initial).clone();
            let output = (self.mapping)(&mut state, value);
            states.insert(change.id.clone(), state);
            TupleChange::put(change.id.clone(), output)
        } else if states.contains_key(&change.id) {
            states.remove(&change.id);
            TupleChange::retract(change.id.clone())
        } else {
            return Ok(vec![]);
        };
        let output = erase([output]);
        self.states = states;
        Ok(output)
    }
}

struct Concat<L, R, T>(PhantomData<(L, R, T)>);
impl<L: Clone + 'static, R: Clone + 'static, T: Clone + 'static, S: AdditiveScore> Node<S>
    for Concat<L, R, T>
{
    fn clear(&mut self) {}
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn restore(&mut self, _: Box<dyn Any>) {}
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = if right {
            let change = typed::<R, T>(event);
            TupleChange {
                id: ConcatId::Right(change.id.clone()),
                value: change.value.clone(),
            }
        } else {
            let change = typed::<L, T>(event);
            TupleChange {
                id: ConcatId::Left(change.id.clone()),
                value: change.value.clone(),
            }
        };
        Ok(erase([change]))
    }
}

struct FlatMap<'a, I, T, U>(FlatMapNode<'a, I, T, U>);
impl<I: Ord + Clone + 'static, T: Clone + 'static, U: 'static, S: AdditiveScore> Node<S>
    for FlatMap<'_, I, T, U>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        Ok(erase(self.0.apply(change.clone())))
    }
}

struct Join<'a, I, J, L, R, K>(JoinNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for Join<'_, I, J, L, R, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
}

struct PartitionedJoin<'a, I, J, L, R, E, K>(PartitionedJoinNode<'a, I, J, L, R, E, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    E: Ord + Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for PartitionedJoin<'_, I, J, L, R, E, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
}

struct OverlapJoin<'a, I, J, L, R, K, E>(OverlapJoinNode<'a, I, J, L, R, K, E>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    E: Ord + Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for OverlapJoin<'_, I, J, L, R, K, E>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
}

struct MembershipJoin<'a, I, J, L, R, K, E>(crate::MembershipJoinNode<'a, I, J, L, R, K, E>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    E: Ord + Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for MembershipJoin<'_, I, J, L, R, K, E>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
}

struct Distinct<I, T>(DistinctNode<I, T>);
impl<I: Ord + Clone + 'static, T: Ord + Clone + 'static, S: AdditiveScore> Node<S>
    for Distinct<I, T>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
    }
}

struct Exists<'a, I, J, L, R, K>(ExistsNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for Exists<'_, I, J, L, R, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0
                .apply_left(typed::<I, L>(event).clone())
                .into_iter()
                .collect()
        }))
    }
}

struct ExistsEqual<'a, I, J, L, R, K>(ExistsEqualNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for ExistsEqual<'_, I, J, L, R, K>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0
                .apply_left(typed::<I, L>(event).clone())
                .into_iter()
                .collect()
        }))
    }
}

struct Reference<'a, I, J, L, R>(ReferenceNode<'a, I, J, L, R>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    S: AdditiveScore,
> Node<S> for Reference<'_, I, J, L, R>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.0.restore(saved);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0
                .apply_left(typed::<I, L>(event).clone())
                .into_iter()
                .collect()
        }))
    }
}

struct Group<'a, I, T, K> {
    node: GroupNode<I, K>,
    mapping: Box<GroupMapping<'a, T, K>>,
}
impl<I: Ord + Clone + 'static, T: 'static, K: Ord + Clone + 'static, S: AdditiveScore> Node<S>
    for Group<'_, I, T, K>
{
    fn clear(&mut self) {
        self.node.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.node.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let mapped = TupleChange {
            id: change.id.clone(),
            value: change.value.as_ref().map(|v| (self.mapping)(v)),
        };
        Ok(erase(self.node.apply(mapped).into_iter().map(|change| {
            let key = change.id.clone();
            change.map(|summary| (key, summary))
        })))
    }
}

type CollectMapping<'a, I, T, K, V> = dyn Fn(&I, &T) -> (K, V) + 'a;
struct Collect<'a, I, T, K, V, C> {
    node: CollectNode<I, K, V, C>,
    mapping: Box<CollectMapping<'a, I, T, K, V>>,
}
impl<
    I: Ord + Clone + 'static,
    T: 'static,
    K: Ord + Clone + 'static,
    V: Clone + 'static,
    C: ReversibleCollector<V> + 'static,
    S: AdditiveScore,
> Node<S> for Collect<'_, I, T, K, V, C>
where
    C::Output: 'static,
{
    fn clear(&mut self) {
        self.node.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.node.restore(c);
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        _arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let mapped = TupleChange {
            id: change.id.clone(),
            value: change.value.as_ref().map(|v| (self.mapping)(&change.id, v)),
        };
        Ok(erase(self.node.apply(mapped).into_iter().map(|c| {
            let key = c.id.clone();
            c.map(|v| (key, v))
        })))
    }
}

type WeightedImpact<'a, T, S> = dyn Fn(
        &T,
        &S,
        crate::ScoringArithmetic,
    ) -> Result<S, crate::scoring_arithmetic::WeightArithmeticError>
    + 'a;
enum TerminalEvaluation<'a, T, S> {
    Direct(Box<dyn Fn(&T) -> S + 'a>),
    Weighted {
        weight: S,
        impact: Box<WeightedImpact<'a, T, S>>,
    },
}
impl<T, S> TerminalEvaluation<'_, T, S> {
    fn evaluate(&self, value: &T, name: &str, arithmetic: NodeArithmetic) -> Result<S, GraphError> {
        match self {
            TerminalEvaluation::Direct(evaluate) => Ok(evaluate(value)),
            TerminalEvaluation::Weighted { weight, impact } => {
                impact(value, weight, arithmetic.impact).map_err(|error| match error {
                    crate::scoring_arithmetic::WeightArithmeticError::Decimal(error) => {
                        GraphError::DecimalWeightArithmetic {
                            constraint: name.to_owned(),
                            error,
                        }
                    }
                    crate::scoring_arithmetic::WeightArithmeticError::Overflow => {
                        GraphError::ScoreOverflow {
                            constraint: Some(name.to_owned()),
                        }
                    }
                })
            }
        }
    }
}
struct TerminalJournal<S> {
    rows: usize,
    total: S,
}
struct Terminal<'a, I, T, S> {
    evaluate: TerminalEvaluation<'a, T, S>,
    total: S,
    name: String,
    matches: SnapshotMap<I, (T, S)>,
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static> Node<S>
    for Terminal<'_, I, T, S>
{
    fn constraint_weight(&self) -> Option<&S> {
        match &self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => Some(weight),
            _ => None,
        }
    }
    fn replace_weight(&mut self, new: S) {
        match &mut self.evaluate {
            TerminalEvaluation::Weighted { weight, .. } => *weight = new,
            _ => unreachable!("unweighted terminal"),
        }
    }

    fn clear(&mut self) {
        self.total = S::ZERO;
        self.matches.clear();
    }
    fn evaluates_score(&self, event: &Event) -> bool {
        typed::<I, T>(event).value.is_some()
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        let journal_view;
        let (old, total) = if let Some(saved) = checkpoint.downcast_ref::<TerminalJournal<S>>() {
            journal_view = self.matches.journal_snapshot(saved.rows);
            (&journal_view, &saved.total)
        } else {
            let (old, total) = checkpoint
                .downcast_ref::<(SnapshotMap<I, (T, S)>, S)>()
                .expect("terminal checkpoint type");
            (old, total)
        };
        self.total == *total
            && self
                .matches
                .iter()
                .map(|(id, (_, s))| (id, s))
                .eq(old.iter().map(|(id, (_, s))| (id, s)))
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.matches.clone(), self.total.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        // User-defined score cloning may panic; it must precede opening a frame.
        let total = self.total.clone();
        Box::new(TerminalJournal {
            rows: self.matches.begin_journal(),
            total,
        })
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<TerminalJournal<S>>()
            .expect("terminal journal");
        self.matches.commit_journal(saved.rows);
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        match c.downcast::<TerminalJournal<S>>() {
            Ok(saved) => {
                self.matches.rollback_journal(saved.rows);
                self.total = saved.total;
            }
            Err(c) => {
                let (matches, total) = *c
                    .downcast::<(SnapshotMap<I, (T, S)>, S)>()
                    .expect("terminal checkpoint type");
                self.matches.install_snapshot(matches);
                self.total = total;
            }
        }
    }
    fn is_constraint(&self) -> bool {
        true
    }
    fn matches(&self) -> Option<&dyn Any> {
        Some(&self.matches)
    }
    fn score(&self) -> S {
        self.total.clone()
    }
    fn apply(
        &mut self,
        _: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let contribution = change
            .value
            .as_ref()
            .map(|value| self.evaluate.evaluate(value, &self.name, arithmetic))
            .transpose()?;
        if contribution
            .as_ref()
            .is_some_and(|score| score.structural() != 0)
        {
            return Err(GraphError::StructuralContribution(self.name.clone()));
        }
        let old = self
            .matches
            .get(&change.id)
            .map_or(S::ZERO, |(_, s)| s.clone());
        let new = contribution.clone().unwrap_or(S::ZERO);
        let total = arithmetic
            .totals
            .adjust(&self.total, [(&old, &new)])
            .map_err(|_| GraphError::ScoreOverflow {
                constraint: Some(self.name.clone()),
            })?;
        if let Some(value) = &change.value {
            self.matches.insert(
                change.id.clone(),
                (value.clone(), contribution.expect("scored match")),
            );
        } else {
            self.matches.remove(&change.id);
        }
        self.total = total;
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy)]
struct Edge {
    node: usize,
    right: bool,
}

/// Build a typed DAG. Nodes only connect to previously created ports, preventing
/// cycles. Payloads/IDs are owned `'static` types; callbacks may borrow immutable
/// problem data for `'a`. All ordinary scores use the same score type `S`.
pub struct StreamGraphBuilder<'a, S: AdditiveScore> {
    owner: Rc<()>,
    nodes: Vec<Box<dyn Node<S> + 'a>>,
    edges: Vec<Vec<Edge>>,
    constraints: BTreeMap<String, usize>,
    datasets: BTreeMap<usize, usize>,
    compound_joins: compound_join::Registrations,
    activity_rules: Vec<ActivityRule>,
    activity_pruning: bool,
    arithmetic: crate::ScoringArithmetic,
}
impl<S: AdditiveScore + 'static> Default for StreamGraphBuilder<'_, S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    pub fn new() -> Self {
        Self {
            owner: Rc::new(()),
            nodes: Vec::new(),
            edges: Vec::new(),
            constraints: BTreeMap::new(),
            datasets: BTreeMap::new(),
            compound_joins: compound_join::Registrations::default(),
            activity_rules: Vec::new(),
            activity_pruning: false,
            arithmetic: crate::ScoringArithmetic::default(),
        }
    }
    /// Set the arithmetic policy for every terminal and the graph total.
    /// Checked built-in integer scoring is the default; wrapping is explicit.
    pub fn with_arithmetic(mut self, arithmetic: crate::ScoringArithmetic) -> Self {
        self.arithmetic = arithmetic;
        self
    }
    fn validate<I, T>(&self, port: &StreamPort<I, T>) -> Result<(), GraphError> {
        if Rc::ptr_eq(&self.owner, &port.owner) {
            Ok(())
        } else {
            Err(GraphError::ForeignPort)
        }
    }
    fn push<I, T>(&mut self, node: impl Node<S> + 'a) -> StreamPort<I, T> {
        let id = self.nodes.len();
        self.nodes.push(Box::new(node));
        self.edges.push(Vec::new());
        self.activity_rules.push(ActivityRule::All);
        StreamPort {
            owner: self.owner.clone(),
            node: id,
            marker: PhantomData,
        }
    }
    fn connect(&mut self, parent: usize, child: usize, right: bool) {
        self.edges[parent].push(Edge { node: child, right });
    }

    pub fn source<I: Ord + Clone + 'static, T: Clone + 'static>(&mut self) -> StreamSource<I, T> {
        StreamSource(self.push(SourceNode::<I, T>(SnapshotMap::new())))
    }
    pub fn filter<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        predicate: impl Fn(&T) -> bool + 'a,
    ) -> Result<StreamPort<I, T>, GraphError> {
        self.validate(input)?;
        let out = self.push(Filter(FilterNode::<I, T>::new(predicate)));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Derive values along dynamic dependencies within a declared key universe.
    /// Every present input row produces `(row, DerivedValue)` under the same key.
    /// Absent targets supply consistent None; cycles and downstream rows are
    /// inconsistent. Equal derived results stop propagation to dependent suppliers.
    /// All cache state participates in trial, undo, saved candidates and rebuilds.
    /// Input dependencies and suppliers must read immutable snapshots only.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, (Option<usize>, i64)>();
    /// let derived = b.derive_dependencies(source.port(), 0..2,
    ///     |row| row.0.into_iter().collect(),
    ///     |_, row, inputs| Some(row.1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0)))?;
    /// let c = b.score("derived", &derived, |(_, v)| SimpleScore::new(v.value.unwrap_or(0)))?;
    /// let mut graph = b.build();
    /// let mut t = graph.transaction();
    /// t.apply(&source, TupleChange::put(0, (None, 3)))?;
    /// t.apply(&source, TupleChange::put(1, (Some(0), 5)))?;
    /// t.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(11));
    /// assert_eq!(graph.constraint_matches(&c)?[&1].0.1.value, Some(8));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn derive_dependencies<
        K: Ord + Clone + fmt::Debug + 'static,
        T: Clone + 'static,
        V: Clone + Eq + 'static,
    >(
        &mut self,
        input: &StreamPort<K, T>,
        keys: impl IntoIterator<Item = K>,
        dependencies: impl Fn(&T) -> Vec<K> + 'a,
        supplier: impl Fn(&K, &T, &[(&K, &crate::DerivedValue<V>)]) -> Option<V> + 'a,
    ) -> Result<StreamPort<K, (T, crate::DerivedValue<V>)>, GraphError> {
        self.validate(input)?;
        let node = crate::DependencyNode::new(keys, dependencies, supplier)
            .map_err(|e| GraphError::Dependency(e.to_string()))?;
        let out = self.push(Derived(node));
        self.connect(input.node, out.node, false);
        Ok(out)
    }

    pub fn map<I: Ord + Clone + 'static, T: Clone + 'static, U: Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> U + 'a,
    ) -> Result<StreamPort<I, U>, GraphError> {
        self.validate(input)?;
        let out = self.push(Map::<I, T, U> {
            mapping: Box::new(mapping),
            marker: PhantomData,
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Project a complete input snapshot using an owned cache per input identity.
    /// The output must be determined solely by the current input and immutable
    /// captured facts. Cached history may reduce work but must not alter results;
    /// applying the input to a fresh initial cache must produce the same output.
    /// Diagnostic rebuilds clear all caches and replay current source rows.
    ///
    /// Cache Clone must produce an independent snapshot (sharing immutable roots
    /// is fine; shared interior mutation is not). Updates clone only the affected
    /// identity's cache; checkpoints share the state-map root. Retraction removes
    /// that identity's cache without invoking the projection. Reinsertion starts
    /// from initial state. Trial undo and saved candidates restore owned caches
    /// without callbacks. External callback side effects cannot be rolled back.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, i64>();
    /// let squares = b.map_cached(source.port(), None::<(i64, i64)>, |cache, value| {
    ///     if cache.as_ref().map(|x| x.0) != Some(*value) {
    ///         *cache = Some((*value, value * value));
    ///     }
    ///     cache.unwrap().1
    /// })?;
    /// b.score("square", &squares, |value| SimpleScore::new(*value))?;
    /// let mut graph = b.build();
    /// let mut t = graph.transaction();
    /// t.apply(&source, TupleChange::put(0, 3))?;
    /// t.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(9));
    /// graph.assert_consistent()?;
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn map_cached<
        I: Ord + Clone + 'static,
        C: Clone + 'static,
        T: Clone + 'static,
        U: Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        initial: C,
        mapping: impl Fn(&mut C, &T) -> U + 'a,
    ) -> Result<StreamPort<I, U>, GraphError> {
        self.validate(input)?;
        let out = self.push(CachedMap::<I, C, T, U> {
            initial,
            states: SnapshotMap::new(),
            mapping: Box::new(mapping),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }

    /// Concatenate two multisets without collapsing duplicates. Branch tags keep
    /// IDs separate, including when `left` and `right` are the same port. The
    /// routing node stores no rows; downstream nodes retain their normal caches
    /// and participate in graph transactions and saved candidate installation.
    ///
    /// Both inputs must have the same payload type. Use `map` with explicit
    /// `Option` fields to pad different tuple arities, or supply custom padding.
    /// Terminal inspection is ordered by branch and ID, not event arrival order.
    ///
    /// ```
    /// use timetuck::{ConcatId, SimpleScore, StreamGraphBuilder, TupleChange};
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, i64>();
    /// let twice = b.concat(source.port(), source.port())?;
    /// let c = b.score("twice", &twice, |v| SimpleScore::new(*v))?;
    /// let mut graph = b.build();
    /// let mut t = graph.transaction();
    /// t.apply(&source, TupleChange::put(7, 3))?;
    /// t.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(6));
    /// assert_eq!(graph.constraint_matches(&c)?.len(), 2);
    /// assert_eq!(graph.constraint_matches(&c)?[&ConcatId::Right(7)].0, 3);
    /// # Ok::<(), timetuck::GraphError>(())
    /// ```
    pub fn concat<L: Ord + Clone + 'static, R: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        left: &StreamPort<L, T>,
        right: &StreamPort<R, T>,
    ) -> Result<ConcatenatedStreamPort<L, R, T>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(Concat::<L, R, T>(PhantomData));
        self.activity_rules[out.node] = ActivityRule::Any;
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Retain every input occurrence and add a padded row for each universe
    /// occurrence whose key has no input match. Equal input keys do not multiply
    /// the padding: the first match hides it and the last retraction restores it.
    /// Duplicate universe keys retain separate IDs when absent from the input.
    /// Input keys outside the universe remain in the result.
    ///
    /// Input occurrences use `ConcatId::Left`; padded universe occurrences use
    /// `ConcatId::Right`. Coverage compares the supplied keys before padding.
    /// Fact discovery, assigned-value filtering and padding are explicit. Both
    /// ports are validated before adding any topology. Internally this composes
    /// count-based equality anti-existence, mapping and concatenation with linear
    /// retained state and transactional rollback guarantees.
    ///
    /// Padding runs only on emitted missing rows, but shared-source updates can
    /// expose transient missing rows before routing drains. It must therefore be
    /// valid for every universe payload, deterministic and free of side effects.
    ///
    /// ```
    /// use timetuck::{SimpleScore, StreamGraphBuilder, TupleChange};
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let matches = b.source::<usize, (u32, i64)>();
    /// let facts = b.source::<usize, u32>();
    /// let complete = b.complement(matches.port(), facts.port(),
    ///     |row| row.0, |fact| *fact, |fact| (*fact, -10))?;
    /// b.score("coverage", &complete, |row| SimpleScore::new(row.1))?;
    /// let mut g = b.build();
    /// let mut t = g.transaction();
    /// t.apply(&facts, TupleChange::put(0, 7))?;
    /// assert_eq!(t.score()?, SimpleScore::new(-10));
    /// t.apply(&matches, TupleChange::put(3, (7, 2)))?;
    /// assert_eq!(t.score()?, SimpleScore::new(2));
    /// t.commit()?;
    /// # Ok::<(), timetuck::GraphError>(())
    /// ```
    pub fn complement<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        T: Clone + 'static,
        U: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        universe: &StreamPort<J, U>,
        input_key: impl Fn(&T) -> K + 'a,
        universe_key: impl Fn(&U) -> K + 'a,
        padding: impl Fn(&U) -> T + 'a,
    ) -> Result<ConcatenatedStreamPort<I, J, T>, GraphError> {
        self.validate(input)?;
        self.validate(universe)?;
        let missing = self.existence_equal(
            universe,
            input,
            ExistenceMode::NotExists,
            universe_key,
            input_key,
        )?;
        let padded = self.map(&missing, padding)?;
        self.concat(input, &padded)
    }
    /// Expand one changed input into a finite ordered sequence. Duplicate payloads
    /// remain separate occurrences with `(input_id, position)` identities. An
    /// update replaces surviving positions and retracts the old tail. Retraction
    /// uses cached lengths without invoking the mapping. Use `distinct` explicitly
    /// when duplicate values should collapse. Tuple closures can retain a prefix
    /// while expanding the last element, or append new elements to the input.
    pub fn flat_map<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        U: Clone + 'static,
        R: IntoIterator<Item = U>,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> R + 'a,
    ) -> Result<StreamPort<(I, usize), U>, GraphError> {
        self.validate(input)?;
        let out = self.push(FlatMap(FlatMapNode::<I, T, U>::new(mapping)));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Follow a single optional reference to a right row's stable identity.
    /// Every left row survives, including absent references and missing targets.
    /// The output preserves its ID and contains `(left, Option<right>)`. Chains
    /// update when either a reference or a target changes; inverse indexes only
    /// visit current dependents. All indexes join ordinary graph rollback and
    /// saved-candidate installation. Graph topology must remain acyclic; cycles
    /// in payload references are allowed but are not recursively evaluated.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let jobs = b.source::<usize, Option<usize>>();
    /// let groups = b.source::<usize, i64>();
    /// let owner = b.follow_reference(jobs.port(), groups.port(), |v| *v)?;
    /// b.score("owner", &owner, |(_, v)| SimpleScore::new(v.unwrap_or(0)))?;
    /// let mut g = b.build();
    /// let mut t = g.transaction();
    /// t.apply(&jobs, TupleChange::put(0, Some(7)))?;
    /// t.apply(&groups, TupleChange::put(7, 42))?;
    /// t.commit()?;
    /// assert_eq!(g.score(), SimpleScore::new(42));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn follow_reference<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        reference: impl Fn(&L) -> Option<J> + 'a,
    ) -> Result<StreamPort<I, (L, Option<R>)>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(Reference(ReferenceNode::<I, J, L, R>::new(reference)));
        self.activity_rules[out.node] = ActivityRule::Left;
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }

    /// Derive one shared value for all left rows referencing the same right ID.
    /// The supplier receives the left row with the smallest tuple ID and the
    /// optional target payload. It runs once for each target change in an occupied
    /// group; targets without followers do not invoke it. Membership changes also
    /// recompute the affected groups. The supplier must return the same intended
    /// result for every member of a group; alignment is an explicit caller promise.
    ///
    /// After each source update finishes routing, every left row has `(left, value)`
    /// under its original ID, including groups with missing targets. Intermediate
    /// routing may retract/reinsert output rows; downstream callbacks must be pure.
    /// Rollback and saved candidates restore caches without rerunning the supplier.
    /// This composes keyed collection and reference indexes; it does not discover
    /// annotation dependencies or cache supplier results across separate updates.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let followers = b.source::<usize, usize>();
    /// let leaders = b.source::<usize, i64>();
    /// let derived = b.follow_shared_reference(followers.port(), leaders.port(),
    ///     |leader| *leader, |_, value| value.copied().unwrap_or(0))?;
    /// b.score("derived", &derived, |(_, value)| SimpleScore::new(*value))?;
    /// let mut graph = b.build();
    /// let mut t = graph.transaction();
    /// t.apply(&followers, TupleChange::put(0, 7))?;
    /// t.apply(&followers, TupleChange::put(1, 7))?;
    /// t.apply(&leaders, TupleChange::put(7, 21))?;
    /// t.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(42));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn follow_shared_reference<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        U: Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        reference: impl Fn(&L) -> J + 'a,
        supplier: impl Fn(&L, Option<&R>) -> U + 'a,
    ) -> Result<StreamPort<I, (L, U)>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let reference = Rc::new(reference);
        let group_key = reference.clone();
        let groups = self.group_list(left, move |row| (group_key(row), row.clone()))?;
        let targets = self.follow_reference(&groups, right, |(key, _)| Some(key.clone()))?;
        let supplied = self.map(&targets, move |((_, members), target)| {
            supplier(&members[0], target.as_ref())
        })?;
        let shared = self.follow_reference(left, &supplied, move |row| Some(reference(row)))?;
        let available = self.filter(&shared, |(_, value)| value.is_some())?;
        self.map(&available, |(row, value)| {
            (row.clone(), value.as_ref().expect("filtered value").clone())
        })
    }

    pub fn join_equal<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(Join(JoinNode::<I, J, L, R, K>::new(left_key, right_key)));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Indexed ordered-key join. Relation orientation is left key versus right
    /// key; duplicate key occurrences remain separate `(left_id, right_id)` pairs.
    /// Equality uses exact bucket lookup, while inequalities use bounded ordered
    /// traversal. Filter the result for additional nonindexed pair predicates.
    ///
    /// ```
    /// use timetuck::{JoinComparison, SimpleScore, StreamGraphBuilder, TupleChange};
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let demand = b.source::<usize, i32>();
    /// let capacity = b.source::<usize, i32>();
    /// let sufficient = b.join_compare(demand.port(), capacity.port(),
    ///     JoinComparison::LessThanOrEqual, |d| *d, |c| *c)?;
    /// let matches = b.score("sufficient", &sufficient, |_| SimpleScore::new(1))?;
    /// let mut graph = b.build();
    /// let mut tx = graph.transaction();
    /// tx.apply(&demand, TupleChange::put(0, 5))?;
    /// tx.apply(&capacity, TupleChange::put(0, 4))?;
    /// tx.apply(&capacity, TupleChange::put(1, 5))?;
    /// tx.commit()?;
    /// assert_eq!(graph.score(), SimpleScore::new(1));
    /// assert_eq!(graph.constraint_matches(&matches)?[&(0, 1)].0, (5, 5));
    /// # Ok::<(), timetuck::GraphError>(())
    /// ```
    pub fn join_compare<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(Join(JoinNode::<I, J, L, R, K>::comparing(
            comparison, left_key, right_key,
        )));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Match equal partitions and compare ordered keys inside each partition.
    /// Projections return `(equality_key, ordered_key)`; tuple equality keys
    /// support multiple equalities. Relation orientation is left versus right.
    /// Only matching partitions/ranges are visited. Additional pair predicates
    /// may be applied with `filter`; tuple ordering is lexicographic.
    pub fn join_partitioned<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        E: Ord + Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(PartitionedJoin(
            PartitionedJoinNode::<I, J, L, R, E, K>::new(comparison, left_key, right_key),
        ));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Indexed strict overlap: `left_start < right_end && right_start < left_end`.
    /// Endpoints are captured without normalization; touching endpoints do not
    /// match. See OverlapJoinNode for degenerate endpoints and pruning behavior.
    pub fn join_overlapping<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_interval: impl Fn(&L) -> (K, K) + 'a,
        right_interval: impl Fn(&R) -> (K, K) + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(OverlapJoin(OverlapJoinNode::<I, J, L, R, K>::new(
            left_interval,
            right_interval,
        )));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Indexed overlap within equal partitions. Projections return
    /// `(equality_key,start,end)`; tuple equality keys support multiple equalities.
    pub fn join_overlapping_partitioned<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
        E: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_interval: impl Fn(&L) -> (E, K, K) + 'a,
        right_interval: impl Fn(&R) -> (E, K, K) + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(OverlapJoin(
            OverlapJoinNode::<I, J, L, R, K, E>::partitioned(left_interval, right_interval),
        ));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Join collections with any common key, emitting each pair once regardless
    /// of duplicate elements or the number of common keys. Empty sets match none.
    pub fn join_containing_any_of<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.join_containing_any_of_partitioned(
            left,
            right,
            move |r| ((), left_keys(r)),
            move |r| ((), right_keys(r)),
        )
    }
    /// Any shared collection key within equal partitions. Projections return an
    /// equality key and collection; tuple equality keys express multiple equalities.
    pub fn join_containing_any_of_partitioned<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
        E: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_keys: impl Fn(&L) -> (E, Vec<K>) + 'a,
        right_keys: impl Fn(&R) -> (E, Vec<K>) + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(MembershipJoin(
            crate::MembershipJoinNode::<I, J, L, R, K, E>::partitioned(left_keys, right_keys),
        ));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Match a left collection containing the projected right scalar. Duplicate
    /// members still emit one pair. Use a singleton in the partitioned helper
    /// when additional equalities are needed.
    pub fn join_containing<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_keys: impl Fn(&L) -> Vec<K> + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.join_containing_any_of(left, right, left_keys, move |r| vec![right_key(r)])
    }
    /// Match a projected left scalar contained in the right collection.
    pub fn join_contained_in<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_keys: impl Fn(&R) -> Vec<K> + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.join_containing_any_of(left, right, move |r| vec![left_key(r)], right_keys)
    }
    /// Emit one representative per value class under `T::cmp`. The lowest input
    /// ID supplies the payload; a representative change propagates an update.
    /// Include fact identity in the value when equal attributes must not collapse.
    pub fn distinct<I: Ord + Clone + 'static, T: Ord + Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
    ) -> Result<StreamPort<T, T>, GraphError> {
        self.validate(input)?;
        let out = self.push(Distinct(DistinctNode::<I, T>::new()));
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// An indexed semi/anti join preserving left identities and payloads. Only
    /// equal-key pairs reach the predicate. Pass `|_, _| true` for pure equality,
    /// constant key functions for a predicate-only relation, or a stable-ID
    /// inequality predicate to exclude an entity from matching itself.
    pub fn existence<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(Exists(ExistsNode::<I, J, L, R, K>::new(
            mode, left_key, right_key, predicate,
        )));
        if mode == ExistenceMode::NotExists {
            self.activity_rules[out.node] = ActivityRule::Left;
        }
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Equality-only semi/anti join with linear retained state. Right updates
    /// visit left rows only when a key gains its first or loses its last match.
    /// Constant keys test any right presence; tuple keys express multiple equalities.
    pub fn existence_equal<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        mode: ExistenceMode,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<StreamPort<I, L>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(ExistsEqual(ExistsEqualNode::<I, J, L, R, K>::new(
            mode, left_key, right_key,
        )));
        if mode == ExistenceMode::NotExists {
            self.activity_rules[out.node] = ActivityRule::Left;
        }
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
    /// Map each input to a group key and integer value. The output carries its key
    /// in both identity and payload so scoring can depend on key properties.
    pub fn group_collect<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, IntSummary)>, GraphError> {
        self.validate(input)?;
        let out = self.push(Group {
            node: GroupNode::<I, K>::new(),
            mapping: Box::new(mapping),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Keyed aggregation with a reversible collector. Only nonempty groups emit.
    pub fn group_collect_with<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
        V: Clone + 'static,
        C: ReversibleCollector<V> + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, V) + 'a,
    ) -> Result<StreamPort<K, (K, C::Output)>, GraphError>
    where
        C::Output: 'static,
    {
        self.validate(input)?;
        let out = self.push(Collect {
            node: CollectNode::<I, K, V, C>::new(),
            mapping: Box::new(move |_, row| mapping(row)),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Aggregate selected values while preserving groups containing rejected rows.
    /// The key and predicate run once per put; value mapping runs only when true.
    /// Removal uses captured decisions/values without invoking these callbacks.
    /// A nonempty group with no selected values emits C's empty result.
    pub fn group_collect_conditionally<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
        V: Clone + 'static,
        C: ReversibleCollector<V> + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        key: impl Fn(&T) -> K + 'a,
        predicate: impl Fn(&T) -> bool + 'a,
        mapping: impl Fn(&T) -> V + 'a,
    ) -> Result<StreamPort<K, (K, C::Output)>, GraphError>
    where
        C::Output: 'static,
    {
        self.group_collect_with::<I, T, K, Option<V>, crate::ConditionalCollector<C>>(
            input,
            move |row| (key(row), predicate(row).then(|| mapping(row))),
        )
    }
    /// Global conditional aggregation: rejected inputs retain the global group.
    /// Only removing every input retracts that group. Selected values alone feed C.
    ///
    /// ```
    /// use timetuck::{CountCollector, SimpleScore, StreamGraphBuilder, TupleChange};
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let s = b.source::<usize, i64>();
    /// let p = b.collect_conditionally::<_, _, _, CountCollector>(
    ///     s.port(), |v| *v > 0, |_| ())?;
    /// let c = b.score("selected", &p, |count| SimpleScore::new(*count as i64))?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&s, TupleChange::put(0, -1))?;
    /// tx.commit()?;
    /// assert_eq!(g.constraint_matches(&c)?.len(), 1); // Retained with count zero.
    /// assert_eq!(g.score(), SimpleScore::ZERO);
    /// # Ok::<(), timetuck::GraphError>(())
    /// ```
    pub fn collect_conditionally<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        V: Clone + 'static,
        C: ReversibleCollector<V> + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        predicate: impl Fn(&T) -> bool + 'a,
        mapping: impl Fn(&T) -> V + 'a,
    ) -> Result<StreamPort<(), C::Output>, GraphError>
    where
        C::Output: 'static,
    {
        let grouped =
            self.group_collect_conditionally::<I, T, (), V, C>(input, |_| (), predicate, mapping)?;
        self.map(&grouped, |(_, result)| result.clone())
    }
    /// Global aggregation. Empty input emits no row, matching Community groupBy;
    /// the standalone collector can still produce its empty result with finish().
    pub fn collect<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        V: Clone + 'static,
        C: ReversibleCollector<V> + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> V + 'a,
    ) -> Result<StreamPort<(), C::Output>, GraphError>
    where
        C::Output: 'static,
    {
        let grouped =
            self.group_collect_with::<I, T, (), V, C>(input, move |row| ((), mapping(row)))?;
        self.map(&grouped, |(_, result)| result.clone())
    }
    /// Collect mapped values per key in ascending input tuple-ID order. Duplicate
    /// values remain separate occurrences; replacements retain their ID position.
    /// Empty groups retract. The native order is independent of delivery order.
    pub fn group_list<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
        V: Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, V) + 'a,
    ) -> Result<GroupedListStreamPort<K, I, V>, GraphError> {
        self.validate(input)?;
        let out = self.push(Collect {
            node: CollectNode::<I, K, (I, V), ListCollector<I, V>>::new(),
            mapping: Box::new(move |id: &I, row| {
                let (key, value) = mapping(row);
                (key, (id.clone(), value))
            }),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
    /// Global list aggregation in ascending input tuple-ID order. Empty input
    /// emits no row. Equal values at different IDs retain their multiplicity.
    pub fn collect_list<I: Ord + Clone + 'static, T: Clone + 'static, V: Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> V + 'a,
    ) -> Result<StreamPort<(), ListSnapshot<I, V>>, GraphError> {
        let grouped = self.group_list(input, move |row| ((), mapping(row)))?;
        self.map(&grouped, |(_, result)| result.clone())
    }
    pub fn score<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        evaluate: impl Fn(&T) -> S + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.validate(input)?;
        let name = name.into();
        if self.constraints.contains_key(&name) {
            return Err(GraphError::DuplicateConstraint(name));
        }
        let out = self.push(Terminal::<I, T, S> {
            evaluate: TerminalEvaluation::Direct(Box::new(evaluate)),
            total: S::ZERO,
            name: name.clone(),
            matches: SnapshotMap::new(),
        });
        self.constraints.insert(name, out.node);
        self.connect(input.node, out.node, false);
        Ok(StreamConstraint(out))
    }
    /// Score each occurrence using a configurable score weight times a signed
    /// integer match factor. Negative factors penalize; positive factors reward.
    /// Scaling is exact and uses the score's native addition/overflow semantics.
    /// Zero weights disable the terminal and exclusively feeding branches at build.
    /// Signed ordinary weights are accepted; structural components are forbidden.
    pub fn score_weighted<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        weight: S,
        factor: impl Fn(&T) -> i64 + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.register_weighted(
            name.into(),
            input,
            weight,
            Box::new(move |value, weight, arithmetic| Ok(arithmetic.scale(weight, factor(value))?)),
        )
    }

    /// Exact fractional match impacts for decimal score families. Product scales
    /// add; no rounding or floating-point conversion is performed. Arithmetic
    /// errors poison the current transaction, which rolls back when dropped.
    /// Named overrides and zero-weight pruning behave as for integer factors.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    /// let source = b.source::<usize, Decimal>();
    /// b.score_weighted_decimal("fraction", source.port(), "0.1".parse().unwrap(), Clone::clone)?;
    /// let mut g = b.build();
    /// let mut tx = g.transaction();
    /// tx.apply(&source, TupleChange::put(0, "0.025".parse().unwrap()))?;
    /// tx.commit()?;
    /// assert_eq!(g.score(), "0.0025".parse().unwrap());
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn score_weighted_decimal<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        input: &StreamPort<I, T>,
        weight: S,
        factor: impl Fn(&T) -> crate::Decimal + 'a,
    ) -> Result<StreamConstraint<I, T>, GraphError>
    where
        S: crate::DecimalWeightedScore,
    {
        self.register_weighted(
            name.into(),
            input,
            weight,
            Box::new(move |value, weight, _| Ok(weight.multiply_exact(&factor(value))?)),
        )
    }

    fn register_weighted<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        name: String,
        input: &StreamPort<I, T>,
        weight: S,
        impact: Box<WeightedImpact<'a, T, S>>,
    ) -> Result<StreamConstraint<I, T>, GraphError> {
        self.validate(input)?;
        if self.constraints.contains_key(&name) {
            return Err(GraphError::DuplicateConstraint(name));
        }
        if weight.structural() != 0 {
            return Err(GraphError::StructuralConstraintWeight(name));
        }
        let out = self.push(Terminal::<I, T, S> {
            evaluate: TerminalEvaluation::Weighted { weight, impact },
            total: S::ZERO,
            name: name.clone(),
            matches: SnapshotMap::new(),
        });
        self.constraints.insert(name, out.node);
        self.connect(input.node, out.node, false);
        Ok(StreamConstraint(out))
    }

    /// Resolve overrides before any source or scoring callbacks run. Unknown
    /// names, unweighted terminals and structural weights are typed errors.
    /// Disabled constraints remain inspectable with zero score and no matches.
    ///
    /// ```
    /// use timetuck::*;
    /// let weights = [("cost".to_owned(), SimpleScore::new(3))].into_iter().collect();
    /// let mut b = StreamGraphBuilder::<SimpleScore>::new();
    /// let source = b.source::<usize, i64>();
    /// b.score_weighted("cost", source.port(), SimpleScore::new(1), |n| -*n)?;
    /// let mut g = b.build_with_weights(&weights)?;
    /// let mut tx = g.transaction();
    /// tx.apply(&source, TupleChange::put(0, 4))?;
    /// tx.commit()?;
    /// assert_eq!(g.score(), SimpleScore::new(-12));
    /// # Ok::<(), GraphError>(())
    /// ```
    pub fn build_with_weights(
        mut self,
        overrides: &crate::ConstraintWeightOverrides<S>,
    ) -> Result<StreamGraph<'a, S>, GraphError> {
        let unknown: Vec<_> = overrides
            .iter()
            .filter(|(name, _)| !self.constraints.contains_key(*name))
            .map(|(name, _)| name.to_owned())
            .collect();
        if !unknown.is_empty() {
            return Err(GraphError::UnknownConstraintWeights {
                unknown,
                known: self.constraints.keys().cloned().collect(),
            });
        }
        for (name, weight) in overrides.iter() {
            let node = &mut self.nodes[self.constraints[name]];
            if node.constraint_weight().is_none() {
                return Err(GraphError::UnweightedConstraintOverride(name.to_owned()));
            }
            if weight.structural() != 0 {
                return Err(GraphError::StructuralConstraintWeight(name.to_owned()));
            }
            node.replace_weight(weight.clone());
        }
        Ok(self.build())
    }

    fn prune_disabled_constraints(&mut self) {
        if !self.nodes.iter().any(|node| {
            node.constraint_weight()
                .is_some_and(|weight| *weight == S::ZERO)
        }) {
            return;
        }
        let mut needed: Vec<_> = self
            .nodes
            .iter()
            .map(|node| node.dataset_rows().is_some())
            .collect();
        for &id in self.constraints.values() {
            needed[id] = self.nodes[id]
                .constraint_weight()
                .is_none_or(|weight| *weight != S::ZERO);
        }
        // IDs are topological: parents always precede their children.
        for id in (0..needed.len()).rev() {
            needed[id] |= self.edges[id].iter().any(|edge| needed[edge.node]);
        }
        for edges in &mut self.edges {
            edges.retain(|edge| needed[edge.node]);
        }
    }

    pub fn build(mut self) -> StreamGraph<'a, S> {
        self.prune_disabled_constraints();
        let mut observed: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                node.dataset_rows().is_some()
                    || (node.is_constraint()
                        && node
                            .constraint_weight()
                            .is_none_or(|weight| *weight != S::ZERO))
            })
            .collect();
        for id in (0..observed.len()).rev() {
            observed[id] |= self.edges[id].iter().any(|edge| observed[edge.node]);
        }
        let batched_updates = self
            .nodes
            .iter()
            .enumerate()
            .any(|(id, node)| node.prefers_batch() && observed[id]);
        let mut graph = StreamGraph {
            batch_workspace: batching::BatchWorkspace::default(),
            transaction_workspaces: Vec::new(),
            batched_updates,
            activity_rules: self.activity_rules,
            active: None,
            owner: self.owner,
            nodes: self.nodes,
            edges: self.edges,
            constraints: self.constraints,
            arithmetic: self.arithmetic,
            score: S::ZERO,
        };
        if self.activity_pruning {
            graph.active = Some(graph.active_plan());
        }
        graph
    }
}

#[derive(Default)]
struct TransactionWorkspace {
    visited: Vec<bool>,
    journal: Vec<(usize, Box<dyn Any>)>,
}

pub struct StreamGraph<'a, S: AdditiveScore> {
    transaction_workspaces: Vec<TransactionWorkspace>,
    batch_workspace: batching::BatchWorkspace,
    batched_updates: bool,
    activity_rules: Vec<ActivityRule>,
    active: Option<Vec<bool>>,
    owner: Rc<()>,
    nodes: Vec<Box<dyn Node<S> + 'a>>,
    edges: Vec<Vec<Edge>>,
    constraints: BTreeMap<String, usize>,
    arithmetic: crate::ScoringArithmetic,
    score: S,
}
impl<'a, S: AdditiveScore> StreamGraph<'a, S> {
    pub fn score(&self) -> S {
        self.score.clone()
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.constraints
            .iter()
            .map(|(name, &id)| (name.as_str(), self.nodes[id].score()))
    }
    pub fn source_rows<I: Ord + 'static, T: 'static>(
        &self,
        source: &StreamSource<I, T>,
    ) -> Result<&SnapshotMap<I, T>, GraphError> {
        self.validate(source)?;
        Ok(self.nodes[source.0.node]
            .source_rows()
            .expect("source handle")
            .downcast_ref()
            .expect("source row type"))
    }
    /// Registered name of a typed constraint, rejecting handles from other graphs.
    pub fn constraint_name<I, T>(
        &self,
        constraint: &StreamConstraint<I, T>,
    ) -> Result<&str, GraphError> {
        if !Rc::ptr_eq(&self.owner, &constraint.0.owner) {
            return Err(GraphError::ForeignPort);
        }
        Ok(self
            .constraints
            .iter()
            .find(|(_, node)| **node == constraint.0.node)
            .expect("registered constraint handle")
            .0
            .as_str())
    }
    pub fn constraint_matches<I: Ord + 'static, T: 'static>(
        &self,
        constraint: &StreamConstraint<I, T>,
    ) -> Result<&SnapshotMap<I, (T, S)>, GraphError>
    where
        S: 'static,
    {
        if !Rc::ptr_eq(&self.owner, &constraint.0.owner) {
            return Err(GraphError::ForeignPort);
        }
        Ok(self.nodes[constraint.0.node]
            .matches()
            .expect("constraint handle")
            .downcast_ref()
            .expect("constraint match type"))
    }
    fn validate<I, T>(&self, source: &StreamSource<I, T>) -> Result<(), GraphError> {
        if Rc::ptr_eq(&self.owner, &source.0.owner) {
            Ok(())
        } else {
            Err(GraphError::ForeignPort)
        }
    }
    /// Rebuild from source rows with empty operator caches and compare every
    /// terminal match ID/score, captured dataset rows and the total. Original state is restored even on
    /// failure/unwind. Diagnostic callback work is excluded from search counters.
    pub fn consistency_report(&mut self) -> Result<crate::StreamConsistencyReport<S>, GraphError> {
        let events: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(id, n)| n.source_events().into_iter().map(move |e| (id, e)))
            .collect();
        let working_nodes: Vec<_> = self.nodes.iter().map(|node| node.score()).collect();
        let names: BTreeMap<_, _> = self
            .constraints
            .iter()
            .map(|(name, node)| (*node, name.clone()))
            .collect();
        let mut trial = self.transaction();
        // Replay order may differ from insertion order. Compare every retained
        // match score after modular rebuilding, avoiding false prefix overflows.
        trial.replaying = true;
        trial.failed = true;
        for id in 0..trial.graph.nodes.len() {
            trial.checkpoint(id);
        }
        for node in &mut trial.graph.nodes {
            node.clear();
        }
        trial.graph.score = S::ZERO;
        if trial.graph.batched_updates {
            trial.replay_sources(events)?;
        } else {
            for (id, event) in events {
                trial.propagate(id, event)?;
            }
        }
        let nodes = trial
            .journal
            .iter()
            .filter_map(|(id, old)| {
                let node = &trial.graph.nodes[*id];
                (!node.scores_match(old.as_ref())).then(|| crate::StreamNodeDifference {
                    node: *id,
                    constraint: names.get(id).cloned(),
                    working_score: working_nodes[*id].clone(),
                    recomputed_score: node.score(),
                })
            })
            .collect();
        Ok(crate::StreamConsistencyReport {
            working_score: trial.original_score.clone(),
            recomputed_score: trial.graph.score.clone(),
            nodes,
        })
    }
    /// Assert the same complete retained-state checks as `consistency_report`,
    /// returning its detached evidence on corruption. Diagnostic rebuilds never
    /// repair caches; graph state is restored on errors and callback panics.
    pub fn assert_consistent(&mut self) -> Result<(), GraphError> {
        let report = self.consistency_report()?;
        if report.is_consistent() {
            Ok(())
        } else {
            Err(GraphError::CorruptedScore(Box::new(report.diagnostic())))
        }
    }
    /// Full state for a multi-move scope; transaction capture contains only the
    /// nodes touched by that single transaction and cannot restore a later edit.
    pub(crate) fn capture_full(&self) -> GraphState<S> {
        GraphState {
            active: self.active.clone(),
            score: self.score.clone(),
            nodes: self
                .nodes
                .iter()
                .enumerate()
                .map(|(id, node)| (id, node.checkpoint()))
                .collect(),
        }
    }
    pub(crate) fn install(&mut self, state: GraphState<S>) {
        for (id, checkpoint) in state.nodes {
            self.nodes[id].restore(checkpoint);
        }
        self.score = state.score;
        self.active = state.active;
    }
    pub fn transaction(&mut self) -> GraphTransaction<'_, 'a, S> {
        let original_score = self.score.clone();
        let original_active = self.active.clone();
        let arithmetic = self.arithmetic;
        // An outer transaction holds its workspace while diagnostics open a
        // nested transaction. Spare workspaces therefore follow nesting depth.
        let mut workspace = self.transaction_workspaces.pop().unwrap_or_default();
        workspace.visited.resize(self.nodes.len(), false);
        let TransactionWorkspace { visited, journal } = workspace;
        GraphTransaction {
            graph: self,
            original_score,
            original_active,
            arithmetic,
            replaying: false,
            visited,
            journal,
            failed: false,
            committed: false,
            score_evaluations: 0,
        }
    }
}

/// An exclusive candidate view. All changes are drained automatically. Drop rolls
/// back, commit keeps the state. After any error or caught callback panic, reads,
/// further updates and commit fail; dropping still restores the original graph.
#[must_use = "dropping a graph transaction rolls it back"]
pub struct GraphTransaction<'g, 'a, S: AdditiveScore> {
    graph: &'g mut StreamGraph<'a, S>,
    original_score: S,
    original_active: Option<Vec<bool>>,
    arithmetic: crate::ScoringArithmetic,
    replaying: bool,
    visited: Vec<bool>,
    journal: Vec<(usize, Box<dyn Any>)>,
    failed: bool,
    committed: bool,
    score_evaluations: u128,
}
pub(crate) struct GraphState<S> {
    active: Option<Vec<bool>>,
    nodes: Vec<(usize, Box<dyn Any>)>,
    score: S,
}
impl<S: AdditiveScore> GraphTransaction<'_, '_, S> {
    pub fn score_evaluations(&self) -> u128 {
        self.score_evaluations
    }
    pub(crate) fn capture(&self) -> Result<GraphState<S>, GraphError> {
        self.valid()?;
        Ok(GraphState {
            active: self.graph.active.clone(),
            score: self.graph.score.clone(),
            nodes: self
                .journal
                .iter()
                .map(|(id, _)| (*id, self.graph.nodes[*id].checkpoint()))
                .collect(),
        })
    }
    pub fn assert_consistent(&mut self) -> Result<(), GraphError> {
        self.valid()?;
        self.failed = true;
        self.graph.assert_consistent()?;
        self.failed = false;
        Ok(())
    }
    fn checkpoint(&mut self, id: usize) {
        if !self.visited[id] {
            let checkpoint = self.graph.nodes[id].begin_checkpoint();
            self.journal.push((id, checkpoint));
            self.visited[id] = true;
        }
    }
    fn valid(&self) -> Result<(), GraphError> {
        if self.failed {
            Err(GraphError::FailedTransaction)
        } else {
            Ok(())
        }
    }
    pub fn score(&self) -> Result<S, GraphError> {
        self.valid()?;
        Ok(self.graph.score.clone())
    }
    pub fn graph(&self) -> Result<&StreamGraph<'_, S>, GraphError> {
        self.valid()?;
        Ok(self.graph)
    }
    /// Number of nodes checkpointed, not the number of copied rows or callbacks.
    pub fn checkpoint_count(&self) -> usize {
        self.journal.len()
    }
    pub fn apply<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        source: &StreamSource<I, T>,
        change: TupleChange<I, T>,
    ) -> Result<(), GraphError> {
        self.valid()?;
        self.failed = true;
        self.graph.validate(source)?;
        let event = Rc::new(change) as Event;
        if self.graph.active.is_some() {
            self.apply_with_activity(source.0.node, event)?;
        } else {
            self.propagate(source.0.node, event)?;
        }
        self.failed = false;
        Ok(())
    }
    fn propagate(&mut self, source: usize, event: Event) -> Result<(), GraphError> {
        let pending = VecDeque::from([(
            Edge {
                node: source,
                right: false,
            },
            event,
        )]);
        self.drain(pending)
    }
    fn drain(&mut self, mut pending: VecDeque<(Edge, Event)>) -> Result<(), GraphError> {
        let arithmetic = NodeArithmetic {
            impact: self.arithmetic,
            totals: if self.replaying {
                crate::ScoringArithmetic::Wrapping
            } else {
                self.arithmetic
            },
        };
        let mut changes = Vec::new();
        while let Some((edge, event)) = pending.pop_front() {
            let id = edge.node;
            if self.graph.active.as_ref().is_some_and(|active| !active[id])
                && self.graph.nodes[id].source_count().is_none()
            {
                continue;
            }
            self.checkpoint(id);
            self.score_evaluations += u128::from(self.graph.nodes[id].evaluates_score(&event));
            let old = self.graph.nodes[id].score();
            let outputs = self.graph.nodes[id].apply(edge.right, &event, arithmetic)?;
            let new = self.graph.nodes[id].score();
            if old != new {
                changes.push((old, new));
            }
            for output in outputs {
                for &next in &self.graph.edges[id] {
                    pending.push_back((next, output.clone()));
                }
            }
        }
        self.graph.score = arithmetic
            .totals
            .adjust(
                &self.graph.score,
                changes.iter().map(|(old, new)| (old, new)),
            )
            .map_err(|_| GraphError::ScoreOverflow { constraint: None })?;
        Ok(())
    }
    pub fn commit(mut self) -> Result<(), GraphError> {
        self.valid()?;
        self.committed = true;
        Ok(())
    }
}
impl<S: AdditiveScore> Drop for GraphTransaction<'_, '_, S> {
    fn drop(&mut self) {
        if !self.committed {
            self.graph.active = self.original_active.take();
            for (id, checkpoint) in self.journal.drain(..).rev() {
                self.visited[id] = false;
                self.graph.nodes[id].restore(checkpoint);
            }
            self.graph.score = std::mem::replace(&mut self.original_score, S::ZERO);
        } else {
            for (id, checkpoint) in self.journal.drain(..).rev() {
                self.visited[id] = false;
                self.graph.nodes[id].commit_checkpoint(checkpoint);
            }
        }
        self.graph
            .transaction_workspaces
            .push(TransactionWorkspace {
                visited: std::mem::take(&mut self.visited),
                journal: std::mem::take(&mut self.journal),
            });
    }
}

pub mod batching;

pub mod batched_existence;

pub mod coalesced;

#[cfg(test)]
mod journal_tests;

mod grouped_batch;

mod filtered_indexed;

mod fixed_links;
pub use fixed_links::FixedLinkedStreamPort;

mod fixed_slots;
mod fixed_terminal;

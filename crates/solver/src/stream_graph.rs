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

use crate::{
    AdditiveScore, CollectNode, DistinctNode, ExistenceMode, ExistsNode, FilterNode, FlatMapNode,
    GroupNode, IntSummary, JoinComparison, JoinNode, ListCollector, ListSnapshot,
    ReversibleCollector, SnapshotMap, TupleChange,
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
    DuplicateConstraint(String),
    StructuralContribution(String),
    FailedTransaction,
    CorruptedScore,
}
impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignPort => f.write_str("stream port belongs to another graph"),
            Self::DuplicateConstraint(name) => write!(f, "duplicate stream constraint: {name}"),
            Self::StructuralContribution(name) => {
                write!(f, "stream constraint {name} has a structural contribution")
            }
            Self::CorruptedScore => {
                f.write_str("cached stream matches or total differ from a fresh rebuild")
            }
            Self::FailedTransaction => {
                f.write_str("stream transaction failed; drop it to roll back")
            }
        }
    }
}
impl std::error::Error for GraphError {}

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

/// Only source handles permit external tuple updates.
pub struct StreamSource<I, T>(StreamPort<I, T>);
impl<I, T> Clone for StreamSource<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<I, T> StreamSource<I, T> {
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

trait Node<S: AdditiveScore> {
    fn checkpoint(&self) -> Box<dyn Any>;
    fn clear(&mut self);
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
    fn apply(&mut self, side: bool, event: &Event) -> Result<Outputs, GraphError>;
    fn score(&self) -> S {
        S::ZERO
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        None
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
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0 = *c.downcast().expect("source checkpoint type");
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        Some(&self.0)
    }
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0.restore(c);
    }
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
        Ok(erase(self.0.apply(typed::<I, T>(event).clone())))
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
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        Ok(erase([TupleChange {
            id: change.id.clone(),
            value: change.value.as_ref().map(|v| (self.mapping)(v)),
        }]))
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
    fn apply(&mut self, right: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, right: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, right: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
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
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
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

struct Terminal<'a, I, T, S> {
    evaluate: Box<dyn Fn(&T) -> S + 'a>,
    total: S,
    name: String,
    matches: SnapshotMap<I, (T, S)>,
}
impl<I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static> Node<S>
    for Terminal<'_, I, T, S>
{
    fn clear(&mut self) {
        self.total = S::ZERO;
        self.matches.clear();
    }
    fn evaluates_score(&self, event: &Event) -> bool {
        typed::<I, T>(event).value.is_some()
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        let (old, total) = checkpoint
            .downcast_ref::<(SnapshotMap<I, (T, S)>, S)>()
            .expect("terminal checkpoint type");
        self.total == *total
            && self
                .matches
                .iter()
                .map(|(id, (_, s))| (id, s))
                .eq(old.iter().map(|(id, (_, s))| (id, s)))
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.matches.clone(), self.total))
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        let (matches, total) = *c
            .downcast::<(SnapshotMap<I, (T, S)>, S)>()
            .expect("terminal checkpoint type");
        self.total = total;
        self.matches = matches;
    }
    fn matches(&self) -> Option<&dyn Any> {
        Some(&self.matches)
    }
    fn score(&self) -> S {
        self.total
    }
    fn apply(&mut self, _: bool, event: &Event) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let contribution = change.value.as_ref().map(|value| (self.evaluate)(value));
        if contribution.is_some_and(|score| score.structural() != 0) {
            return Err(GraphError::StructuralContribution(self.name.clone()));
        }
        let total = self.total - self.matches.get(&change.id).map_or(S::ZERO, |(_, s)| *s)
            + contribution.unwrap_or(S::ZERO);
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
        }
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
    /// indexed anti-existence, mapping and concatenation, including their cached
    /// match-link memory cost and transactional rollback guarantees.
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
        let missing = self.existence(
            universe,
            input,
            ExistenceMode::NotExists,
            universe_key,
            input_key,
            |_, _| true,
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
            evaluate: Box::new(evaluate),
            total: S::ZERO,
            name: name.clone(),
            matches: SnapshotMap::new(),
        });
        self.constraints.insert(name, out.node);
        self.connect(input.node, out.node, false);
        Ok(StreamConstraint(out))
    }
    pub fn build(self) -> StreamGraph<'a, S> {
        StreamGraph {
            owner: self.owner,
            nodes: self.nodes,
            edges: self.edges,
            constraints: self.constraints,
            score: S::ZERO,
        }
    }
}

pub struct StreamGraph<'a, S: AdditiveScore> {
    owner: Rc<()>,
    nodes: Vec<Box<dyn Node<S> + 'a>>,
    edges: Vec<Vec<Edge>>,
    constraints: BTreeMap<String, usize>,
    score: S,
}
impl<'a, S: AdditiveScore> StreamGraph<'a, S> {
    pub fn score(&self) -> S {
        self.score
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
    /// terminal match ID/score and the total. Original state is restored even on
    /// failure/unwind. Diagnostic callback work is excluded from search counters.
    pub fn assert_consistent(&mut self) -> Result<(), GraphError> {
        let events: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(id, n)| n.source_events().into_iter().map(move |e| (id, e)))
            .collect();
        let mut trial = self.transaction();
        trial.failed = true;
        for id in 0..trial.graph.nodes.len() {
            trial.checkpoint(id);
        }
        for node in &mut trial.graph.nodes {
            node.clear();
        }
        trial.graph.score = S::ZERO;
        for (id, event) in events {
            trial.propagate(id, event)?;
        }
        if trial.graph.score != trial.original_score
            || trial
                .journal
                .iter()
                .any(|(id, old)| !trial.graph.nodes[*id].scores_match(old.as_ref()))
        {
            return Err(GraphError::CorruptedScore);
        }
        Ok(())
    }
    pub(crate) fn install(&mut self, state: GraphState<S>) {
        for (id, checkpoint) in state.nodes {
            self.nodes[id].restore(checkpoint);
        }
        self.score = state.score;
    }
    pub fn transaction(&mut self) -> GraphTransaction<'_, 'a, S> {
        let original_score = self.score;
        let visited = vec![false; self.nodes.len()];
        GraphTransaction {
            graph: self,
            original_score,
            visited,
            journal: Vec::new(),
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
    visited: Vec<bool>,
    journal: Vec<(usize, Box<dyn Any>)>,
    failed: bool,
    committed: bool,
    score_evaluations: u128,
}
pub(crate) struct GraphState<S> {
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
            score: self.graph.score,
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
            let checkpoint = self.graph.nodes[id].checkpoint();
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
        Ok(self.graph.score)
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
        self.propagate(source.0.node, Rc::new(change) as Event)?;
        self.failed = false;
        Ok(())
    }
    fn propagate(&mut self, source: usize, event: Event) -> Result<(), GraphError> {
        let mut pending = VecDeque::from([(
            Edge {
                node: source,
                right: false,
            },
            event,
        )]);
        while let Some((edge, event)) = pending.pop_front() {
            let id = edge.node;
            self.checkpoint(id);
            self.score_evaluations += u128::from(self.graph.nodes[id].evaluates_score(&event));
            let old = self.graph.nodes[id].score();
            let outputs = self.graph.nodes[id].apply(edge.right, &event)?;
            self.graph.score = self.graph.score - old + self.graph.nodes[id].score();
            for output in outputs {
                for &next in &self.graph.edges[id] {
                    pending.push_back((next, output.clone()));
                }
            }
        }
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
            for (id, checkpoint) in self.journal.drain(..).rev() {
                self.graph.nodes[id].restore(checkpoint);
            }
            self.graph.score = self.original_score;
        }
    }
}

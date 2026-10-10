//! Restricted graph edits for facts owned by a retained score director.
use super::{batching::StreamBatch, *};

/// An atomic set of external fact updates. Updates reach the existing joins,
/// collectors and score terminals. Planning sources cannot be changed, including
/// through a batch; their assignment and derived shadows stay synchronized.
///
/// Drop/unwind restores cached state without replaying callbacks. Any update
/// error or caught callback panic poisons the transaction, preventing commit of
/// a partial change. Fact identities and values must follow `StreamGraph`'s
/// immutable snapshot contract. A committed change takes effect in the next
/// retained phase/solve; previous results and acceptance history are not reused.
///
/// ```
/// use timetuck::*;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)])?;
/// let mut builder = StreamGraphBuilder::<SimpleScore>::new();
/// let planning = builder.source::<usize, (usize, Option<i32>)>();
/// let costs = builder.source::<i32, (i32, i64)>();
/// let joined = builder.join_equal(planning.port(), costs.port(),
///     |(_, value)| *value, |(value, _)| Some(*value))?;
/// builder.score("cost", &joined, |(_, (_, cost))| SimpleScore::new(-cost))?;
/// let mut director = StreamScoreDirector::new(
///     &problem, builder.build(), planning, problem.initial_assignment())?;
/// let mut facts = director.edit_facts();
/// facts.apply(&costs, TupleChange::put(1, (1, 30)))?;
/// assert_eq!(facts.score()?, SimpleScore::new(-30));
/// facts.commit()?;
/// assert_eq!(director.score(), SimpleScore::new(-30));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use = "dropping a fact transaction rolls it back"]
pub struct FactTransaction<'g, 'a, S: AdditiveScore> {
    transaction: GraphTransaction<'g, 'a, S>,
    protected: Vec<usize>,
    structural: i64,
}
impl<'g, 'a, S: AdditiveScore> FactTransaction<'g, 'a, S> {
    pub(crate) fn new(
        graph: &'g mut StreamGraph<'a, S>,
        protected: Vec<usize>,
        structural: i64,
    ) -> Self {
        Self {
            transaction: graph.transaction(),
            protected,
            structural,
        }
    }
    /// Current score including the unchanged assignment's structural component.
    pub fn score(&self) -> Result<S, GraphError> {
        Ok(self.transaction.score()?.with_structural(self.structural))
    }
    pub fn graph(&self) -> Result<&StreamGraph<'_, S>, GraphError> {
        self.transaction.graph()
    }
    pub fn score_evaluations(&self) -> u128 {
        self.transaction.score_evaluations()
    }
    pub fn checkpoint_count(&self) -> usize {
        self.transaction.checkpoint_count()
    }
    pub fn assert_consistent(&mut self) -> Result<(), GraphError> {
        self.transaction.assert_consistent()
    }
    pub fn apply<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        source: &StreamSource<I, T>,
        change: TupleChange<I, T>,
    ) -> Result<(), GraphError> {
        self.transaction.valid()?;
        self.transaction.failed = true;
        self.transaction.graph.validate(source)?;
        if self.protected.contains(&source.0.node) {
            return Err(GraphError::ProtectedPlanningSource);
        }
        self.transaction.failed = false;
        self.transaction.apply(source, change)
    }
    /// Settle heterogeneous sources together with normal graph batch semantics.
    /// Validate every source before propagating any part of the batch.
    pub fn apply_batch(&mut self, batch: StreamBatch) -> Result<(), GraphError> {
        self.transaction.valid()?;
        self.transaction.failed = true;
        if batch
            .owner
            .as_ref()
            .is_some_and(|owner| !Rc::ptr_eq(owner, &self.transaction.graph.owner))
        {
            return Err(GraphError::ForeignPort);
        }
        if batch
            .events
            .iter()
            .any(|(node, _)| self.protected.contains(node))
        {
            return Err(GraphError::ProtectedPlanningSource);
        }
        self.transaction.failed = false;
        self.transaction.apply_batch(batch)
    }
    pub fn commit(self) -> Result<(), GraphError> {
        self.transaction.commit()
    }
}

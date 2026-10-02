//! Bind a transactional stream graph to finite basic-variable moves.
//!
//! Planning rows have stable variable-index IDs and `(index, Option<value>)`
//! payloads. All variables, including unassigned ones, are present. Constraints
//! explicitly filter unassigned rows. Other graph sources are immutable during
//! the scoring session. Existing construction/local-search entry points still
//! accept scoped networks; their generic stream binding remains future work.
//!
//! ```
//! use timetuck::{FiniteProblem, Variable, SimpleScore, StreamGraphBuilder,
//!     StreamScoreDirector, Move};
//! let problem = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])])?;
//! let mut builder = StreamGraphBuilder::<SimpleScore>::new();
//! let source = builder.source::<usize, (usize, Option<i64>)>();
//! builder.score("value", source.port(), |(_, value)| SimpleScore::new(value.unwrap_or(0)))?;
//! let mut director = StreamScoreDirector::new(
//!     &problem, builder.build(), source, problem.initial_assignment())?;
//! assert_eq!(director.score(), SimpleScore::ZERO.with_structural(-1));
//! {
//!     let trial = director.apply(&Move::Change { variable: 0, value: Some(2) })?;
//!     assert_eq!(trial.score(), SimpleScore::new(2));
//! } // Both the assignment and graph roll back.
//! assert_eq!(director.assignment().values(), &[None]);
//! director.apply(&Move::Change { variable: 0, value: Some(1) })?.commit();
//! assert_eq!(director.score(), SimpleScore::new(1));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::scoring::with_structure;
use crate::{
    AdditiveScore, AppliedMove, Assignment, FiniteProblem, GraphError, GraphTransaction,
    ModelError, Move, StreamGraph, StreamSource, TupleChange,
};
use std::{error::Error, fmt};

pub type PlanningStreamSource<V> = StreamSource<usize, (usize, Option<V>)>;

#[derive(Debug)]
pub enum StreamScoringError {
    Model(ModelError),
    Graph(GraphError),
    PlanningSourceNotEmpty,
}
impl fmt::Display for StreamScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::PlanningSourceNotEmpty => {
                f.write_str("planning stream source must be empty before binding")
            }
        }
    }
}
impl Error for StreamScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::PlanningSourceNotEmpty => None,
        }
    }
}
impl From<ModelError> for StreamScoringError {
    fn from(e: ModelError) -> Self {
        Self::Model(e)
    }
}
impl From<GraphError> for StreamScoringError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

pub struct StreamScoreDirector<'p, 'a, V, S: AdditiveScore> {
    problem: &'p FiniteProblem<V>,
    assignment: Assignment<V>,
    graph: StreamGraph<'a, S>,
    source: PlanningStreamSource<V>,
}
impl<'p, 'a, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    StreamScoreDirector<'p, 'a, V, S>
{
    pub fn new(
        problem: &'p FiniteProblem<V>,
        mut graph: StreamGraph<'a, S>,
        source: PlanningStreamSource<V>,
        assignment: Assignment<V>,
    ) -> Result<Self, StreamScoringError> {
        problem.validate(&assignment)?;
        if !graph.source_rows(&source)?.is_empty() {
            return Err(StreamScoringError::PlanningSourceNotEmpty);
        }
        let mut transaction = graph.transaction();
        for (index, value) in assignment.values().iter().enumerate() {
            transaction.apply(&source, TupleChange::put(index, (index, value.clone())))?;
        }
        transaction.commit()?;
        Ok(Self {
            problem,
            assignment,
            graph,
            source,
        })
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn graph(&self) -> &StreamGraph<'a, S> {
        &self.graph
    }
    pub fn score(&self) -> S {
        with_structure(self.graph.score(), self.problem, &self.assignment)
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.graph.constraint_scores()
    }

    /// Validate the final composite assignment, then deliver each touched variable
    /// once using its final value. Drop/ordinary unwind restore both assignments
    /// and graph state. Commit performs no callback replay.
    pub fn apply(
        &mut self,
        movement: &Move<V>,
    ) -> Result<StreamScoredMove<'_, 'a, V, S>, StreamScoringError> {
        let assignment = self.problem.apply(&mut self.assignment, movement)?;
        let mut graph = self.graph.transaction();
        for variable in movement.touched_variables() {
            graph.apply(
                &self.source,
                TupleChange::put(variable, (variable, assignment.values()[variable].clone())),
            )?;
        }
        let score = with_structure(graph.score()?, self.problem, &assignment);
        Ok(StreamScoredMove {
            assignment,
            graph,
            score,
        })
    }
}

#[must_use = "dropping a scored stream move restores the assignment and graph"]
pub struct StreamScoredMove<'g, 'a, V, S: AdditiveScore> {
    assignment: AppliedMove<'g, V>,
    graph: GraphTransaction<'g, 'a, S>,
    score: S,
}
impl<V, S: AdditiveScore> StreamScoredMove<'_, '_, V, S> {
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score
    }
    pub fn graph(&self) -> &StreamGraph<'_, S> {
        self.graph.graph().expect("successfully scored transaction")
    }
    pub fn checkpoint_count(&self) -> usize {
        self.graph.checkpoint_count()
    }
    pub fn commit(self) {
        self.graph
            .commit()
            .expect("successfully scored transaction");
        self.assignment.commit();
    }
}

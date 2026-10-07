//! Detached assertions over a complete cached score and named contributions.
use crate::{AdditiveScore, ConstraintComparison, GraphError, StreamGraph};
use std::{collections::BTreeMap, fmt};

/// An owned observation. The total is kept separately from named contributions:
/// a director's total may also contain structural penalties for unassigned values.
/// Capture both from the same state; construction does not rescore or validate a
/// caller-supplied total. Names must be unique, including zero-weight constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreAssertion<S> {
    score: S,
    constraints: BTreeMap<String, S>,
}
impl<S> ScoreAssertion<S> {
    /// Supports scoped or stream directors through their score/constraint_scores
    /// accessors. This does not infer omitted constraints or repair input facts.
    pub fn from_scores<N: Into<String>>(
        score: S,
        contributions: impl IntoIterator<Item = (N, S)>,
    ) -> Result<Self, GraphError> {
        let mut constraints = BTreeMap::new();
        for (name, value) in contributions {
            let name = name.into();
            if constraints.insert(name.clone(), value).is_some() {
                return Err(GraphError::DuplicateConstraint(name));
            }
        }
        Ok(Self { score, constraints })
    }
    pub fn score(&self) -> &S {
        &self.score
    }
    pub fn constraints(&self) -> &BTreeMap<String, S> {
        &self.constraints
    }
}

/// Failure retains typed scores and every named contribution. Display limits
/// contribution output to five entries; the structured data is never truncated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreAssertionError<S> {
    pub expected: ConstraintComparison<S>,
    pub actual: S,
    pub context: Option<String>,
    pub constraints: BTreeMap<String, S>,
}
impl<S: fmt::Debug> fmt::Display for ScoreAssertionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (relation, score) = match &self.expected {
            ConstraintComparison::Exactly(s) => ("", s),
            ConstraintComparison::MoreThan(s) => (" more than", s),
            ConstraintComparison::LessThan(s) => (" less than", s),
        };
        write!(
            f,
            "{}; Expected score{relation} {score:?}; Actual score {:?}",
            self.context.as_deref().unwrap_or("Broken expectation"),
            self.actual
        )?;
        for (name, score) in self.constraints.iter().take(5) {
            write!(f, "; {name}: {score:?}")?;
        }
        if self.constraints.len() > 5 {
            write!(f, "; {} more constraints", self.constraints.len() - 5)?;
        }
        Ok(())
    }
}
impl<S: fmt::Debug> std::error::Error for ScoreAssertionError<S> {}
impl<S: Clone + Ord> ScoreAssertion<S> {
    /// Compare using the score type's equality and order (including structural
    /// levels). This never compares formatted text or converts scores to floats.
    pub fn check(
        &self,
        expected: ConstraintComparison<S>,
        context: Option<&str>,
    ) -> Result<(), Box<ScoreAssertionError<S>>> {
        let pass = match &expected {
            ConstraintComparison::Exactly(s) => self.score == *s,
            ConstraintComparison::MoreThan(s) => self.score > *s,
            ConstraintComparison::LessThan(s) => self.score < *s,
        };
        if pass {
            return Ok(());
        }
        Err(Box::new(ScoreAssertionError {
            expected,
            actual: self.score.clone(),
            context: context.map(str::to_owned),
            constraints: self.constraints.clone(),
        }))
    }
}

/// Capture every registered graph constraint and its cached total without invoking
/// scoring callbacks. Graph scores exclude structural penalties added by an outer
/// director. Use `ScoreAssertion::from_scores` with that director's accessors when
/// asserting the entire solution score including those penalties.
///
/// ```
/// use timetuck::*;
/// let mut b = StreamGraphBuilder::<HardSoftScore>::new();
/// let source = b.source::<usize, i64>();
/// b.score("cost", source.port(), |v| HardSoftScore::new(0, -*v))?;
/// let mut graph = b.build();
/// let mut tx = graph.transaction();
/// tx.apply(&source, TupleChange::put(0, 3))?;
/// tx.commit()?;
/// let a = score_assertion(&graph);
/// a.check(ConstraintComparison::Exactly(HardSoftScore::new(0, -3)), Some("one job"))?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn score_assertion<S: AdditiveScore>(graph: &StreamGraph<'_, S>) -> ScoreAssertion<S> {
    ScoreAssertion {
        score: graph.score(),
        constraints: graph
            .constraint_scores()
            .map(|(name, score)| (name.to_owned(), score))
            .collect(),
    }
}

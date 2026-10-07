//! Typed evidence from an isolated rebuild of retained stream state.
//!
//! ```
//! use std::cell::Cell;
//! use timetuck::*;
//! let factor = Cell::new(1_i64);
//! let mut b = StreamGraphBuilder::<SimpleScore>::new();
//! let source = b.source::<usize, i64>();
//! b.score("cost", source.port(), |v| SimpleScore::new(v * factor.get()))?;
//! let mut graph = b.build();
//! let mut tx = graph.transaction();
//! tx.apply(&source, TupleChange::put(0, 3))?;
//! tx.commit()?;
//! factor.set(2); // Deliberately violate the immutable-fact contract.
//! let report = graph.consistency_report()?;
//! assert!(!report.is_consistent());
//! assert_eq!(report.nodes[0].constraint.as_deref(), Some("cost"));
//! assert_eq!(report.recomputed_score, SimpleScore::new(6));
//! assert_eq!(graph.score(), SimpleScore::new(3)); // Report does not repair caches.
//! # Ok::<(), GraphError>(())
//! ```
use std::fmt;

/// One graph-local node whose checked state changed. A named node checks terminal
/// match identities and their scores; an unnamed node checks a captured dataset's
/// identities and payloads. Equal aggregate scores do not imply equal match state.
/// Node indices identify this graph only, not corresponding nodes in other graphs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamNodeDifference<S> {
    pub node: usize,
    pub constraint: Option<String>,
    pub working_score: S,
    pub recomputed_score: S,
}
/// The graph's total and every changed terminal/dataset, captured before restoring
/// the original state. Intermediate operator caches are rebuilt but are not compared
/// directly. Terminal tuple payloads are not compared unless captured in a dataset.
/// User callback side effects are outside graph rollback and must be controlled by
/// the caller. A successful rebuild does not prove the objective itself correct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamConsistencyReport<S> {
    pub working_score: S,
    pub recomputed_score: S,
    pub nodes: Vec<StreamNodeDifference<S>>,
}
impl<S: Eq> StreamConsistencyReport<S> {
    pub fn is_consistent(&self) -> bool {
        self.working_score == self.recomputed_score && self.nodes.is_empty()
    }
}
impl<S: fmt::Debug> StreamConsistencyReport<S> {
    /// Format detached evidence for the nongeneric graph error. Consistency must
    /// be decided using typed equality, not these strings.
    pub fn diagnostic(&self) -> StreamScoreCorruption {
        StreamScoreCorruption {
            working_score: format!("{:?}", self.working_score),
            recomputed_score: format!("{:?}", self.recomputed_score),
            nodes: self
                .nodes
                .iter()
                .map(|n| StreamNodeDifference {
                    node: n.node,
                    constraint: n.constraint.clone(),
                    working_score: format!("{:?}", n.working_score),
                    recomputed_score: format!("{:?}", n.recomputed_score),
                })
                .collect(),
        }
    }
}
/// Complete formatted evidence; Display includes at most five node differences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamScoreCorruption {
    pub working_score: String,
    pub recomputed_score: String,
    pub nodes: Vec<StreamNodeDifference<String>>,
}
impl fmt::Display for StreamScoreCorruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Score corruption in stream graph: working score {}; recomputed score {}",
            self.working_score, self.recomputed_score
        )?;
        for n in self.nodes.iter().take(5) {
            match &n.constraint {
                Some(name) => write!(
                    f,
                    "; constraint {name:?} (node {}): retained matches or total differ, cached {}, recomputed {}",
                    n.node, n.working_score, n.recomputed_score
                )?,
                None => write!(f, "; dataset node {}: retained rows differ", n.node)?,
            }
        }
        if self.nodes.len() > 5 {
            write!(f, "; {} more node differences", self.nodes.len() - 5)?;
        }
        Ok(())
    }
}
impl std::error::Error for StreamScoreCorruption {}

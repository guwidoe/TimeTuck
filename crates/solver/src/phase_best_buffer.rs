//! Bounded snapshot buffering for phase-level best-solution delivery.
use crate::{Score, SolverPhaseKind, SolverUpdate};

/// Owned retained improvement, preserving native phase origin and completeness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferedSolverUpdate<A, S> {
    pub phase: usize,
    pub kind: SolverPhaseKind,
    pub assignment: A,
    pub score: S,
    pub complete: bool,
}
/// Retain only the latest improvement from each observed phase. observe returns
/// the preceding phase's pending snapshot on an origin change; flush returns the
/// current pending snapshot. Always flush after a successful solve to deliver a
/// final partial result when construction ends early. A flush is idempotent until
/// another improvement arrives. No background task or automatic Drop callback.
///
/// The caller supplies the scored initial state's baseline. Equal/worse updates
/// are ignored, except that a complete assignment supersedes a partial baseline,
/// matching native solver retention. The initial score alone is never published.
/// Use a fresh buffer per solve or problem revision; it does not reset implicitly.
///
/// This optional delivery adapter leaves the solver's immediate observation and
/// scoring behavior unchanged. It owns at most one pending assignment snapshot,
/// clones only accepted improvements, and preserves native phase metadata. It
/// does not claim Java notification timing or asynchronous queue semantics.
///
/// ```
/// use timetuck::*;
/// let mut buffer = PhaseBestBuffer::new(SimpleScore::new(0), true);
/// let assignment = vec![1];
/// assert!(buffer.observe(SolverUpdate {phase:0,kind:SolverPhaseKind::LocalSearch,
///     assignment:&assignment,score:SimpleScore::new(1),complete:true}).is_none());
/// assert_eq!(buffer.flush().unwrap().assignment, vec![1]);
/// assert!(buffer.flush().is_none());
/// ```
pub struct PhaseBestBuffer<A, S> {
    best: S,
    complete: bool,
    origin: Option<(usize, SolverPhaseKind)>,
    pending: Option<BufferedSolverUpdate<A, S>>,
}
impl<A: Clone, S: Score> PhaseBestBuffer<A, S> {
    pub fn new(initial_score: S, initial_complete: bool) -> Self {
        Self {
            best: initial_score,
            complete: initial_complete,
            origin: None,
            pending: None,
        }
    }
    pub fn observe(
        &mut self,
        update: SolverUpdate<'_, A, S>,
    ) -> Option<BufferedSolverUpdate<A, S>> {
        let origin = (update.phase, update.kind);
        // Clone accepted state before consuming a pending delivery, so a failed
        // clone does not silently lose the previous phase's retained snapshot.
        let replacement = if (!self.complete && update.complete) || update.score > self.best {
            Some(BufferedSolverUpdate {
                phase: update.phase,
                kind: update.kind,
                assignment: update.assignment.clone(),
                score: update.score.clone(),
                complete: update.complete,
            })
        } else {
            None
        };
        let ready = if self.origin.is_some_and(|old| old != origin) {
            self.pending.take()
        } else {
            None
        };
        self.origin = Some(origin);
        if let Some(replacement) = replacement {
            self.best = update.score;
            self.complete = update.complete;
            self.pending = Some(replacement);
        }
        ready
    }
    pub fn flush(&mut self) -> Option<BufferedSolverUpdate<A, S>> {
        self.pending.take()
    }
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

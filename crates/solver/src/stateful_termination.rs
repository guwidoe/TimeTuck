//! History-dependent stopping expressions, separate from stateless score queries.
use crate::{
    Score, ScoredSearchTermination, SearchProgress, SignificantImprovement,
    SignificantImprovementConfig, TerminationExpression, TerminationState,
};
use std::{error::Error, fmt, time::Duration};

/// A stateless subtree or an independent score-difference stagnation history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatefulSearchLimit<'a, S = crate::SimpleScore> {
    Static(&'a ScoredSearchTermination<'a, S>),
    /// Uses the validated threshold's exact comparison and strictly-after-window
    /// expiry. Every occurrence has its own anchor, including repeated leaves.
    SignificantImprovement(SignificantImprovementConfig<S>),
}
/// AND/OR composition with retained-score history. Bind solve-wide through
/// [`crate::SolverExtensions::stateful_termination`] or to an individual phase
/// with [`crate::PhaseTerminationBinding::stateful`]. All history occurrences
/// receive score observations and elapsed samples, even if another branch has
/// already decided the expression's Boolean result.
///
/// Solve history arms on the first complete retained score after search starts.
/// Initial preparation and construction cannot arm it. Once armed, elapsed time
/// continues through later preparation and construction, but construction scores
/// do not update its anchor. Phase bindings create fresh independent histories.
/// Static subtrees retain the usual solve/phase counter and complete-score scope.
///
/// ```
/// use timetuck::*;
/// use std::time::Duration;
/// let moves = ScoredSearchTermination::Condition(
///     ScoredSearchLimit::Search(SearchLimit::MoveEvaluations(10_000)));
/// let children = [
///     StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&moves)),
///     StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
///         SignificantImprovementConfig::new(Duration::from_secs(30), SimpleScore::new(100))?)),
/// ];
/// let stop = StatefulSearchTermination::All(&children);
/// assert_eq!(stop.try_status(SearchProgress::default(), None),
///     Err(StatefulTerminationError::HistoryRequired));
/// // Pass Some(&stop) in SolverExtensions::stateful_termination, or bind a phase.
/// let binding = PhaseTerminationBinding::stateful(0, &stop);
/// assert_eq!(binding.phase(), 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type StatefulSearchTermination<'a, S = crate::SimpleScore> =
    TerminationExpression<'a, StatefulSearchLimit<'a, S>>;

/// A stateless query cannot evaluate a history-dependent expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatefulTerminationError {
    HistoryRequired,
}
impl fmt::Display for StatefulTerminationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("termination requires a StatefulTerminationSession")
    }
}
impl Error for StatefulTerminationError {}
impl<S: Score> StatefulSearchTermination<'_, S> {
    pub fn try_status(
        &self,
        progress: SearchProgress,
        best: Option<&S>,
    ) -> Result<Option<TerminationState>, StatefulTerminationError> {
        if self.needs_history() {
            return Err(StatefulTerminationError::HistoryRequired);
        }
        Ok(self.evaluate(|leaf| match leaf {
            StatefulSearchLimit::Static(condition) => condition.status(progress, best),
            StatefulSearchLimit::SignificantImprovement(_) => unreachable!("checked history"),
        }))
    }
    fn any_leaf(&self, predicate: impl Fn(&StatefulSearchLimit<'_, S>) -> bool + Copy) -> bool {
        match self {
            Self::Condition(leaf) => predicate(leaf),
            Self::All(children) | Self::Any(children) => {
                children.iter().any(|c| c.any_leaf(predicate))
            }
        }
    }
    pub(crate) fn needs_history(&self) -> bool {
        self.any_leaf(|leaf| matches!(leaf, StatefulSearchLimit::SignificantImprovement(_)))
    }
    pub(crate) fn needs_best(&self) -> bool {
        self.any_leaf(|leaf| matches!(leaf, StatefulSearchLimit::Static(c) if c.needs_best_score()))
    }
    pub(crate) fn needs_scores(&self) -> bool {
        self.any_leaf(
            |leaf| matches!(leaf, StatefulSearchLimit::Static(c) if c.needs_score_counts()),
        )
    }
    pub(crate) fn needs_moves(&self) -> bool {
        self.any_leaf(
            |leaf| matches!(leaf, StatefulSearchLimit::Static(c) if c.needs_move_counts()),
        )
    }
}

/// One invocation's score histories. The expression remains immutable and can
/// be reused by other independent sessions. Call `start_search` after setup,
/// then `observe_best` for complete retained scores from search phases only.
/// `status` polls history expiry but never treats its static best-score argument
/// as a new history observation. This separates construction's retained goals
/// from search improvement anchors without rescoring or assignment snapshots.
///
/// Elapsed readings clamp monotonically. Each history's expiry latches; the
/// expression itself is evaluated afresh (integrated solver/phase controls latch
/// the whole expression). `reset` discards all history and search arming.
#[derive(Debug)]
pub struct StatefulTerminationSession<'a, S = crate::SimpleScore> {
    expression: &'a StatefulSearchTermination<'a, S>,
    histories: Vec<SignificantImprovement<S>>,
    started: bool,
    last: Duration,
}
impl<'a, S: Score> StatefulTerminationSession<'a, S> {
    pub fn new(expression: &'a StatefulSearchTermination<'a, S>) -> Self {
        let mut histories = Vec::new();
        expression.evaluate(|leaf| {
            if let StatefulSearchLimit::SignificantImprovement(config) = leaf {
                histories.push(SignificantImprovement::new(config.clone()));
            }
            None
        });
        Self {
            expression,
            histories,
            started: false,
            last: Duration::ZERO,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.expression);
    }
    pub fn start_search(&mut self, elapsed: Duration) {
        self.last = self.last.max(elapsed);
        self.started = true;
    }
    /// Call only for complete retained search results. Structurally flawed scores
    /// are ignored; accepted worsening states never reset a threshold anchor.
    pub fn observe_best(&mut self, elapsed: Duration, score: &S) {
        self.last = self.last.max(elapsed);
        if !self.started || score.structural() < 0 {
            return;
        }
        for history in &mut self.histories {
            if history.is_started() {
                history.is_terminated(self.last, score);
            } else {
                history.start(self.last, score.clone());
            }
        }
    }
    pub fn status(
        &mut self,
        mut progress: SearchProgress,
        best: Option<&S>,
    ) -> Option<TerminationState> {
        self.last = self.last.max(progress.elapsed);
        progress.elapsed = self.last;
        let mut histories = self.histories.iter_mut();
        self.expression.evaluate(|leaf| match leaf {
            StatefulSearchLimit::Static(condition) => condition.status(progress, best),
            StatefulSearchLimit::SignificantImprovement(_) => {
                let history = histories.next().expect("one history per occurrence");
                let reached = history.expired(self.last);
                Some(TerminationState {
                    reached,
                    progress: history.is_started().then(|| history.progress(self.last)),
                })
            }
        })
    }
}

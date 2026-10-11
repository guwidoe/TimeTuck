//! Validated score-goal progress with invocation-local initialized baselines.
use crate::{
    AdditiveScore, FeasibilityProgress, Score, ScoreProgressError, ScoreTargetProgress,
    TerminationState,
};

/// Weighted progress toward a score target or feasibility. Bind through
/// [`crate::StatefulSearchLimit::ScoreGoal`]. The session captures its first
/// complete, structurally sound initialized score; the configuration never fixes
/// a starting score. Split weights allocate fractions of the remaining share,
/// exactly as [`ScoreTargetProgress`] and [`FeasibilityProgress`].
///
/// Solve scope captures the first complete retained observation, including a
/// construction initialization or committed placement. Phase scope uses complete
/// retained input when available, otherwise its first complete observation.
/// Construction's intermediate scores seed only the baseline: current/retained
/// best selection remains the normal solver policy, including optional worsening.
///
/// ```
/// use timetuck::*;
/// let goal = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
///     ScoreGoalProgressConfig::target(HardSoftScore::new(0, 100), vec![0.75])?));
/// let mut session = StatefulTerminationSession::new(&goal);
/// session.observe_initialized_score(&HardSoftScore::new(-10, 0));
/// let halfway = session.status(SearchProgress::default(), Some(&HardSoftScore::new(-5, 50))).unwrap();
/// assert_eq!(halfway.progress, Some(0.5));
/// assert!(!halfway.reached);
/// // Bind goal in SolverExtensions::stateful_termination, optionally selecting
/// // Acceptance::SimulatedAnnealingTermination to cool from this progress.
/// # Ok::<(), ScoreProgressError>(())
/// ```
///
/// Exact score/feasibility checks decide termination, independently of rounded or
/// zero-weight progress. Before initialization, the leaf is false with no estimate.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreGoalProgressConfig<S> {
    target: Option<S>,
    splits: Vec<f64>,
}
// Constructors reject NaN and infinity; fields are private.
impl<S: Eq> Eq for ScoreGoalProgressConfig<S> {}
impl<S: Score> ScoreGoalProgressConfig<S> {
    /// N ordinary score levels require N-1 finite split weights in [0,1].
    pub fn target(target: S, splits: Vec<f64>) -> Result<Self, ScoreProgressError> {
        // Reuse existing validation, including custom score shape semantics.
        ScoreTargetProgress::validate_target(&target, &splits)?;
        Ok(Self {
            target: Some(target),
            splits,
        })
    }
    pub(crate) fn capture(&self, start: &S) -> GoalProgress<S> {
        match &self.target {
            Some(target) => GoalProgress::Target(
                ScoreTargetProgress::new(start.clone(), target.clone(), self.splits.clone())
                    .expect("validated fixed score shape"),
            ),
            None => GoalProgress::Feasible(
                FeasibilityProgress::new(start.clone(), self.splits.clone())
                    .expect("validated fixed score shape"),
            ),
        }
    }
}
impl<S: AdditiveScore> ScoreGoalProgressConfig<S> {
    /// H hard levels require H-1 weights (zero for H=0). Soft levels are ignored.
    /// S::ZERO validates shape only; it is never used as the session's baseline.
    pub fn feasibility(splits: Vec<f64>) -> Result<Self, ScoreProgressError> {
        FeasibilityProgress::new(S::ZERO, splits.clone())?;
        Ok(Self {
            target: None,
            splits,
        })
    }
}
#[derive(Debug)]
pub(crate) enum GoalProgress<S> {
    Target(ScoreTargetProgress<S>),
    Feasible(FeasibilityProgress<S>),
}
impl<S: Score> GoalProgress<S> {
    pub(crate) fn status(&self, best: &S) -> TerminationState {
        match self {
            Self::Target(p) => TerminationState {
                reached: p.is_reached_ref(best),
                progress: Some(p.progress_ref(best)),
            },
            Self::Feasible(p) => TerminationState {
                reached: p.is_reached_ref(best),
                progress: Some(p.progress_ref(best)),
            },
        }
    }
}

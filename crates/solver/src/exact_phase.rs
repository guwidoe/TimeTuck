//! Optional move-based exhaustive phase with observed real branch restoration.
use crate::{
    BranchRestoreError, BranchRestoreMode, BranchRestorer, NodeExploration, PruningFrontier,
    RecordedMove, Score, SearchNodeKey,
};
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactPhaseKind {
    BranchAndBound,
    BruteForce,
}
#[derive(Clone, Copy, Debug)]
pub struct ExactMovePhaseConfig {
    pub kind: ExactPhaseKind,
    pub exploration: Option<NodeExploration>,
    pub max_steps: usize,
}
impl Default for ExactMovePhaseConfig {
    fn default() -> Self {
        Self {
            kind: ExactPhaseKind::BranchAndBound,
            exploration: None,
            max_steps: usize::MAX,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidExactExploration(pub NodeExploration);
fn exploration_name(order: NodeExploration) -> &'static str {
    match order {
        NodeExploration::DepthFirst => "DEPTH_FIRST",
        NodeExploration::BreadthFirst => "BREADTH_FIRST",
        NodeExploration::ScoreFirst => "SCORE_FIRST",
        NodeExploration::OptimisticBoundFirst => "OPTIMISTIC_BOUND_FIRST",
        NodeExploration::InputOrderDepthFirst => "ORIGINAL_ORDER",
    }
}
impl fmt::Display for InvalidExactExploration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The phaseConfig (ExhaustiveSearchPhaseConfig) has an nodeExplorationType ({}) which is not compatible with its exhaustiveSearchType (BRUTE_FORCE).",
            exploration_name(self.0)
        )
    }
}
impl Error for InvalidExactExploration {}
impl ExactMovePhaseConfig {
    pub fn validate(&self) -> Result<NodeExploration, InvalidExactExploration> {
        if self.kind == ExactPhaseKind::BruteForce {
            if let Some(order) = self.exploration {
                return Err(InvalidExactExploration(order));
            }
            Ok(NodeExploration::InputOrderDepthFirst)
        } else {
            Ok(self.exploration.unwrap_or(NodeExploration::DepthFirst))
        }
    }
}
/// An explicit native branching model. Moves are ordered finite choices against
/// the current node, not precomputed desired successor states. Each application
/// must be transactional on error; inverse moves must restore its exact preimage.
/// Bounds must be admissible initialized scores; None disables pruning for a node.
/// `restored` supports explicit scoring/shadow work at a completed restoration.
pub trait ExactPhaseModel {
    type Assignment: Eq + Clone;
    type Move: Clone;
    type Score: Score;
    type Error: fmt::Debug;
    fn initial(&mut self) -> Result<Self::Assignment, Self::Error>;
    fn max_depth(&self) -> usize;
    /// Adapter-defined scheduling layer; repeated layers support ordered list
    /// owners with several insertions, independently of completion depth.
    fn child_depth(
        &self,
        parent_depth: usize,
        _movement: &Self::Move,
        _child: &Self::Assignment,
    ) -> usize {
        parent_depth + 1
    }
    fn is_terminal(&self, depth: usize, _working: &Self::Assignment) -> bool {
        depth >= self.max_depth()
    }

    fn moves(
        &mut self,
        depth: usize,
        working: &Self::Assignment,
    ) -> Result<Vec<Self::Move>, Self::Error>;
    fn apply(
        &mut self,
        working: &mut Self::Assignment,
        movement: &Self::Move,
    ) -> Result<(), Self::Error>;
    fn inverse(&self, before: &Self::Assignment, after: &Self::Assignment) -> Self::Move;
    fn score(&mut self, working: &Self::Assignment) -> Result<Self::Score, Self::Error>;
    fn optimistic_bound(
        &mut self,
        _depth: usize,
        _working: &Self::Assignment,
        _score: &Self::Score,
    ) -> Result<Option<Self::Score>, Self::Error> {
        Ok(None)
    }
    fn restore_mode(&self) -> BranchRestoreMode {
        BranchRestoreMode::CommonAncestor
    }
    /// Observe actual root/record identity after a completed expansion. Root
    /// arenas are independent even when their physical starting states are equal.
    fn expanded(
        &mut self,
        _root: usize,
        _node: usize,
        _depth: usize,
        _working: &Self::Assignment,
        _score: &Self::Score,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    fn restored(&mut self, _working: &Self::Assignment) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct ExactPhaseBest<A, S> {
    pub assignment: A,
    pub score: S,
}
#[derive(Clone, Debug)]
pub struct ExactPhaseResult<A, S> {
    pub working: A,
    pub best: ExactPhaseBest<A, S>,
    pub steps: usize,
    pub generated: usize,
}
#[derive(Debug)]
pub enum ExactPhaseError<E> {
    Configuration(InvalidExactExploration),
    InvalidRootCount,
    InitialScoreState,
    Model(E),
    Restore(BranchRestoreError<E>),
}
impl<E: fmt::Debug> fmt::Display for ExactPhaseError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact phase: {self:?}")
    }
}
impl<E: fmt::Debug> Error for ExactPhaseError<E> {}
pub type ExactPhaseOutcome<A, S, E> = Result<ExactPhaseResult<A, S>, ExactPhaseError<E>>;
/// Expand nonterminal frontier nodes under a step ceiling. Each completed step
/// observes the restored expanding assignment; terminal children improve the best
/// result during generation and are never counted as steps. Ties retain the first
/// best witness. All node identities, global sibling breadths and moves are real.
/// Existing snapshot-based exhaustive defaults are unchanged. Observer panics occur
/// after successful work and propagate. Caller bounds/branching establish quality.
pub fn run_exact_phase<M: ExactPhaseModel>(
    model: &mut M,
    config: ExactMovePhaseConfig,
    step: impl FnMut(&M::Assignment, &M::Score),
) -> ExactPhaseOutcome<M::Assignment, M::Score, M::Error> {
    run_seeded_exact_phase(model, config, None, 1, step)
}
/// A genuinely calculated score tied to its exact assignment preimage. Cached
/// initial scoring is opt-in; matching state alone cannot validate application
/// facts captured by the calculator, which the caller must keep unchanged.
#[derive(Clone, Debug)]
pub struct ExactPhaseScoreCache<A, S> {
    pub assignment: A,
    pub score: S,
}
/// Seed several declared root entries into one frontier. Identical root contexts
/// retain distinct scheduling breadths, share pruning/best state and consume one
/// step budget. This is not sequential rerunning. Existing single-root defaults
/// retain their original scoring and lifecycle behavior.
pub fn run_seeded_exact_phase<M: ExactPhaseModel>(
    model: &mut M,
    config: ExactMovePhaseConfig,
    cached: Option<ExactPhaseScoreCache<M::Assignment, M::Score>>,
    root_count: usize,
    mut step: impl FnMut(&M::Assignment, &M::Score),
) -> ExactPhaseOutcome<M::Assignment, M::Score, M::Error> {
    let order = config.validate().map_err(ExactPhaseError::Configuration)?;
    if root_count == 0 {
        return Err(ExactPhaseError::InvalidRootCount);
    }
    let mut working = model.initial().map_err(ExactPhaseError::Model)?;
    let score = if let Some(cache) = cached {
        if cache.assignment != working {
            return Err(ExactPhaseError::InitialScoreState);
        }
        cache.score
    } else {
        model.score(&working).map_err(ExactPhaseError::Model)?
    };
    let mut best = ExactPhaseBest {
        assignment: working.clone(),
        score: score.clone(),
    };
    if model.is_terminal(0, &working) {
        return Ok(ExactPhaseResult {
            working,
            best,
            steps: 0,
            generated: 0,
        });
    }
    let mut restorers: Vec<_> = (0..root_count)
        .map(|_| BranchRestorer::new(working.clone()))
        .collect();
    let mut current_root = 0;
    let mut frontier = PruningFrontier::new(order);
    let bound = if config.kind == ExactPhaseKind::BranchAndBound {
        model
            .optimistic_bound(0, &working, &score)
            .map_err(ExactPhaseError::Model)?
    } else {
        None
    };
    for root in 0..root_count {
        frontier.push(
            SearchNodeKey {
                depth: 0,
                score: score.clone(),
                optimistic_bound: bound.clone(),
                parent_breadth: 0,
                breadth: root as u64,
            },
            (root, 0usize),
        );
    }
    let mut steps = 0;
    let mut generated = 0;
    let mut breadth = (root_count - 1) as u64;
    while steps < config.max_steps {
        let Some((key, (root, node))) = frontier.pop() else {
            break;
        };
        let mode = model.restore_mode();
        if root != current_root {
            restorers[current_root]
                .restore(0, mode, &mut working, |a, m| model.apply(a, m), |_, _| {})
                .map_err(ExactPhaseError::Restore)?;
            current_root = root;
        }
        restorers[root]
            .restore(
                node,
                mode,
                &mut working,
                |a, m| model.apply(a, m),
                |_, _| {},
            )
            .map_err(ExactPhaseError::Restore)?;
        model.restored(&working).map_err(ExactPhaseError::Model)?;
        for movement in model
            .moves(key.depth, &working)
            .map_err(ExactPhaseError::Model)?
        {
            let mut child = working.clone();
            model
                .apply(&mut child, &movement)
                .map_err(ExactPhaseError::Model)?;
            let score = model.score(&child).map_err(ExactPhaseError::Model)?;
            generated += 1;
            breadth = breadth
                .checked_add(1)
                .expect("exact phase breadth overflow");
            let child_depth = model.child_depth(key.depth, &movement, &child);
            if model.is_terminal(child_depth, &child) {
                if score > best.score {
                    best = ExactPhaseBest {
                        assignment: child,
                        score: score.clone(),
                    };
                    if config.kind == ExactPhaseKind::BranchAndBound && score.structural() >= 0 {
                        frontier.register_bound(score).expect("initialized bound");
                    }
                }
                continue;
            }
            let bound = if config.kind == ExactPhaseKind::BranchAndBound {
                model
                    .optimistic_bound(child_depth, &child, &score)
                    .map_err(ExactPhaseError::Model)?
            } else {
                None
            };
            let undo = model.inverse(&working, &child);
            let record = RecordedMove::from_parts(working.clone(), child, movement, undo);
            let child_node = restorers[root]
                .add_child(node, record)
                .expect("child built from current node");
            frontier.push(
                SearchNodeKey {
                    depth: child_depth,
                    score,
                    optimistic_bound: bound,
                    parent_breadth: key.breadth,
                    breadth,
                },
                (root, child_node),
            );
        }
        steps += 1;
        model
            .expanded(root, node, key.depth, &working, &key.score)
            .map_err(ExactPhaseError::Model)?;
        step(&working, &key.score);
    }
    Ok(ExactPhaseResult {
        working,
        best,
        steps,
        generated,
    })
}

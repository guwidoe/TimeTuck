//! Per-run candidate counts; no allocations or monitor locks on increment.
use crate::{ListMove, MixedMove, Move};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Native evaluated-candidate families. A composite is counted once, not once
/// per child. Mixed moves with an empty component use the other component's
/// family; moves with both components use Mixed. Exact search scores assignments
/// or tree nodes, not Java change moves. Custom commands count attempted operations
/// (including inspected flawed previews); reconstruction counts scored candidates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(usize)]
pub enum MoveEvaluationKind {
    Change,
    Swap,
    Composite,
    ListAssign,
    ListChange,
    ListSwap,
    SubListChange,
    SubListSwap,
    ListUnassign,
    ListComposite,
    Mixed,
    ExactAssignment,
    ExactNode,
    Custom,
    Reconstruction,
    FineRepair,
}
impl MoveEvaluationKind {
    pub const ALL: [Self; 16] = [
        Self::Change,
        Self::Swap,
        Self::Composite,
        Self::ListAssign,
        Self::ListChange,
        Self::ListSwap,
        Self::SubListChange,
        Self::SubListSwap,
        Self::ListUnassign,
        Self::ListComposite,
        Self::Mixed,
        Self::ExactAssignment,
        Self::ExactNode,
        Self::Custom,
        Self::Reconstruction,
        Self::FineRepair,
    ];
}
/// Cumulative counts for one invocation, including rejected scored candidates.
/// Initialization and assertion/diagnostic rescoring are excluded. Each counter
/// is atomic; a snapshot during concurrent evaluation may straddle an increment.
/// Counters saturate at u64::MAX. Reconstruction publishes after each rebuild;
/// failed rebuilds have no partial counts. Other scored search candidates publish
/// after successful scoring, while custom operations publish when attempted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MoveEvaluationCounts([u64; 16]);
impl MoveEvaluationCounts {
    pub fn get(&self, kind: MoveEvaluationKind) -> u64 {
        self.0[kind as usize]
    }
    pub fn total(&self) -> u64 {
        self.0.iter().fold(0u64, |n, v| n.saturating_add(*v))
    }
    pub fn iter(&self) -> impl Iterator<Item = (MoveEvaluationKind, u64)> + '_ {
        MoveEvaluationKind::ALL
            .into_iter()
            .map(|k| (k, self.get(k)))
    }
}
#[derive(Clone, Default)]
pub(crate) struct MoveCounter(Arc<[AtomicU64; 16]>);
impl MoveCounter {
    pub(crate) fn add(&self, kind: MoveEvaluationKind, n: u64) {
        let _ = self.0[kind as usize].fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.saturating_add(n))
        });
    }
    pub(crate) fn snapshot(&self) -> MoveEvaluationCounts {
        MoveEvaluationCounts(std::array::from_fn(|i| self.0[i].load(Ordering::Relaxed)))
    }
}
pub(crate) fn from_control<S>(
    shared: &Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Option<MoveCounter> {
    shared.as_ref().and_then(|s| s.move_counter())
}
pub(crate) trait CountedMove {
    fn metric_kind(&self) -> MoveEvaluationKind;
}
pub(crate) fn record<M: CountedMove>(counter: &Option<MoveCounter>, movement: &M) {
    if let Some(counter) = counter {
        counter.add(movement.metric_kind(), 1);
    }
}
impl<V> CountedMove for Move<V> {
    fn metric_kind(&self) -> MoveEvaluationKind {
        match self {
            Self::Change { .. } => MoveEvaluationKind::Change,
            Self::Swap { .. } => MoveEvaluationKind::Swap,
            Self::Composite(_) => MoveEvaluationKind::Composite,
        }
    }
}
impl<V> CountedMove for ListMove<V> {
    fn metric_kind(&self) -> MoveEvaluationKind {
        match self {
            Self::Assign { .. } => MoveEvaluationKind::ListAssign,
            Self::Change { .. } => MoveEvaluationKind::ListChange,
            Self::Swap { .. } => MoveEvaluationKind::ListSwap,
            Self::SubListChange { .. } => MoveEvaluationKind::SubListChange,
            Self::SubListSwap { .. } => MoveEvaluationKind::SubListSwap,
            Self::Unassign { .. } => MoveEvaluationKind::ListUnassign,
            Self::Composite(_) => MoveEvaluationKind::ListComposite,
        }
    }
}
impl<B, L> CountedMove for MixedMove<B, L> {
    fn metric_kind(&self) -> MoveEvaluationKind {
        if matches!(&self.lists, ListMove::Composite(v) if v.is_empty()) {
            self.basic.metric_kind()
        } else if matches!(&self.basic, Move::Composite(v) if v.is_empty()) {
            self.lists.metric_kind()
        } else {
            MoveEvaluationKind::Mixed
        }
    }
}

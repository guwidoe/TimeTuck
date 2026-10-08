//! Round-based existence with explicit downstream lifecycle events.
use super::{ExistsNode, State};
use crate::{ExistenceMode, SnapshotMap, TupleChange};

/// When a batched existence node evaluates keys and its predicate. Both choices
/// publish downstream changes only from `propagate` and have identical round
/// results for pure callbacks. Both evaluate inputs in enqueue order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExistenceEvaluation {
    Eager,
    Deferred,
}

/// A balanced lifecycle transition for a single output occurrence. Unlike an
/// upsert, Insert requires an absent ID and Update requires an existing ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TupleLifecycleEvent<I, T> {
    Insert { id: I, value: T },
    Update { id: I, value: T },
    Retract { id: I },
}
impl<I, T> TupleLifecycleEvent<I, T> {
    pub fn id(&self) -> &I {
        match self {
            Self::Insert { id, .. } | Self::Update { id, .. } | Self::Retract { id } => id,
        }
    }
    pub fn into_change(self) -> TupleChange<I, T> {
        match self {
            Self::Insert { id, value } | Self::Update { id, value } => TupleChange::put(id, value),
            Self::Retract { id } => TupleChange::retract(id),
        }
    }
}
enum Input<I, J, L, R> {
    Left(TupleChange<I, L>),
    Right(TupleChange<J, R>),
}
#[derive(Clone)]
struct Prepared<I, J, L, R, K> {
    state: State<I, J, L, R, K>,
    dirty: SnapshotMap<I, ()>,
}
/// Incremental semi/anti join with explicit round boundaries. Inputs retain their
/// enqueue order. Each successful propagation emits at most one lifecycle event
/// per affected left ID, sorted by ID, reflecting the final visible state.
/// A transient insert followed by removal emits nothing. A previously visible
/// row removed and reinserted in the same round emits Update, preserving identity.
/// Multiple witnesses never multiply a left row. Unchanged right witnesses emit
/// no event; visible left replacements refresh their payload without requiring Eq.
///
/// Committed rows stay visible through `rows` until propagation succeeds. Eager
/// mode stages private persistent roots when enqueueing; deferred mode retains
/// inputs and evaluates them on propagation. `discard_pending` abandons a round.
/// Callback/clone/comparison panics leave committed state unchanged. Failed eager
/// enqueues leave earlier pending work intact; failed deferred propagations retain
/// all inputs for retry. External callback effects are not rolled back.
///
/// The equality index restricts comparisons to matching keys; a constant key
/// permits an arbitrary predicate. Matching pair links can use quadratic space.
/// This standalone node is not the stream graph's immediate scheduling policy.
///
/// ```
/// use timetuck::*;
/// let mut n=BatchedExistsNode::new(ExistenceMode::Exists,ExistenceEvaluation::Deferred,
///     |_: &i32| (),|_: &i32| (),|l,r|l>r);
/// n.enqueue_left(TupleChange::put(0,3));
/// n.enqueue_right(TupleChange::put(0,2));
/// assert_eq!(n.rows().count(),0);
/// assert_eq!(n.propagate(),vec![TupleLifecycleEvent::Insert{id:0,value:3}]);
/// n.enqueue_right(TupleChange::retract(0));
/// n.discard_pending();
/// assert!(n.propagate().is_empty());
/// ```
pub struct BatchedExistsNode<'a, I, J, L, R, K> {
    node: ExistsNode<'a, I, J, L, R, K>,
    evaluation: ExistenceEvaluation,
    pending: Vec<Input<I, J, L, R>>,
    staged: Option<Prepared<I, J, L, R, K>>,
    pending_count: usize,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    BatchedExistsNode<'a, I, J, L, R, K>
{
    pub fn new(
        mode: ExistenceMode,
        evaluation: ExistenceEvaluation,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Self {
        Self {
            node: ExistsNode::new(mode, left_key, right_key, predicate),
            evaluation,
            pending: Vec::new(),
            staged: None,
            pending_count: 0,
        }
    }
    fn trial(&self, state: State<I, J, L, R, K>) -> ExistsNode<'_, I, J, L, R, K> {
        ExistsNode {
            state,
            mode: self.node.mode,
            left_key: Box::new(|row| (self.node.left_key)(row)),
            right_key: Box::new(|row| (self.node.right_key)(row)),
            predicate: Box::new(|l, r| (self.node.predicate)(l, r)),
        }
    }
    fn apply(
        node: &mut ExistsNode<'_, I, J, L, R, K>,
        dirty: &mut SnapshotMap<I, ()>,
        input: Input<I, J, L, R>,
    ) {
        match input {
            Input::Left(change) => {
                if let Some(change) = node.apply_left(change) {
                    dirty.insert(change.id, ());
                }
            }
            Input::Right(change) => {
                for change in node.apply_right(change) {
                    dirty.insert(change.id, ());
                }
            }
        }
    }
    fn enqueue(&mut self, input: Input<I, J, L, R>) {
        let count = self
            .pending_count
            .checked_add(1)
            .expect("existence pending count overflow");
        match self.evaluation {
            ExistenceEvaluation::Deferred => self.pending.push(input),
            ExistenceEvaluation::Eager => {
                let prepared = self.staged.clone().unwrap_or_else(|| Prepared {
                    state: self.node.state.clone(),
                    dirty: SnapshotMap::new(),
                });
                let mut dirty = prepared.dirty;
                let state = {
                    let mut trial = self.trial(prepared.state);
                    Self::apply(&mut trial, &mut dirty, input);
                    trial.state
                };
                self.staged = Some(Prepared { state, dirty });
            }
        }
        self.pending_count = count;
    }
    pub fn enqueue_left(&mut self, change: TupleChange<I, L>) {
        self.enqueue(Input::Left(change));
    }
    pub fn enqueue_right(&mut self, change: TupleChange<J, R>) {
        self.enqueue(Input::Right(change));
    }
    pub fn pending_len(&self) -> usize {
        self.pending_count
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &L)> {
        self.node.rows()
    }
    pub fn discard_pending(&mut self) {
        self.pending.clear();
        self.staged = None;
        self.pending_count = 0;
    }
    pub fn propagate(&mut self) -> Vec<TupleLifecycleEvent<I, L>> {
        if self.pending_count == 0 {
            return Vec::new();
        }
        let prepared = match self.evaluation {
            ExistenceEvaluation::Eager => {
                self.staged.as_ref().expect("pending eager state").clone()
            }
            ExistenceEvaluation::Deferred => {
                let mut trial = self.trial(self.node.state.clone());
                let mut dirty = SnapshotMap::new();
                for input in &self.pending {
                    let input = match input {
                        Input::Left(c) => Input::Left(c.clone()),
                        Input::Right(c) => Input::Right(c.clone()),
                    };
                    Self::apply(&mut trial, &mut dirty, input);
                }
                Prepared {
                    state: trial.state,
                    dirty,
                }
            }
        };
        let visible = |state: &State<I, J, L, R, K>, id: &I| {
            state.left.rows.contains_key(id) && self.node.mode.accepts(state.left.matched(id))
        };
        let mut out = Vec::new();
        for id in prepared.dirty.keys() {
            let before = visible(&self.node.state, id);
            let after = visible(&prepared.state, id);
            match (before, after) {
                (false, true) => out.push(TupleLifecycleEvent::Insert {
                    id: id.clone(),
                    value: prepared.state.left.rows[id].1.clone(),
                }),
                (true, true) => out.push(TupleLifecycleEvent::Update {
                    id: id.clone(),
                    value: prepared.state.left.rows[id].1.clone(),
                }),
                (true, false) => out.push(TupleLifecycleEvent::Retract { id: id.clone() }),
                (false, false) => {}
            }
        }
        self.node.state = prepared.state;
        self.discard_pending();
        out
    }
}

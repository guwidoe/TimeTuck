//! Atomic join rounds with coalesced inputs and one predicate check per pair.
use super::{JoinComparison, JoinSide};
use crate::{SnapshotMap, TupleChange, TupleLifecycleEvent};

/// Eager evaluates each input on enqueue; Deferred evaluates only the last value
/// of each changed ID at propagation. Both publish only final round events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoinEvaluation {
    Eager,
    Deferred,
}
#[derive(Clone)]
struct State<I, J, L, R, K> {
    left: JoinSide<I, L, K>,
    right: JoinSide<J, R, K>,
    pairs: SnapshotMap<(I, J), (L, R)>,
}
impl<I, J, L, R, K> State<I, J, L, R, K> {
    fn new() -> Self {
        Self {
            left: JoinSide::new(),
            right: JoinSide::new(),
            pairs: SnapshotMap::new(),
        }
    }
}
#[derive(Clone)]
struct Prepared<I, J, L, R, K> {
    state: State<I, J, L, R, K>,
    dirty: SnapshotMap<(I, J), ()>,
}
type Predicate<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;
type Events<I, J, L, R> = Vec<TupleLifecycleEvent<(I, J), (L, R)>>;

/// Equality-indexed, filtered join with atomic round publication. Tuple equality
/// keys express multiple equalities; a constant key allows arbitrary predicates.
/// Deferred rounds keep only the latest input for each ID, update both indexes,
/// then check each affected final candidate pair once. Eager mode evaluates every
/// input as it arrives, on private roots. Intermediate values may therefore call
/// callbacks only in Eager mode; callbacks must be pure and stable.
///
/// Propagation emits one Insert/Update/Retract per affected pair, ordered by pair
/// ID. Surviving pairs refresh both payloads; transient pairs disappear from the
/// output. `rows` exposes committed pairs until propagation succeeds. Panicking
/// projections, predicates, comparisons or clones preserve committed state and
/// earlier pending work. Failed deferred rounds can be retried; discard_pending
/// abandons the round. External side effects and panicking destructors are excluded.
///
/// Pair payloads and indexes use persistent roots. Storage includes all accepted
/// pairs and can be quadratic for dense joins. This standalone batching policy
/// does not replace the stream graph's immediate scheduler.
///
/// ```
/// use timetuck::*;
/// let mut n=BatchedJoinNode::new(JoinEvaluation::Deferred,
///     |r:&i32|r%2,|r:&i32|r%2,|l,r|l<r);
/// n.enqueue_left(TupleChange::put(0,1));
/// n.enqueue_left(TupleChange::put(0,3));
/// n.enqueue_right(TupleChange::put(0,5));
/// assert_eq!(n.propagate(),vec![TupleLifecycleEvent::Insert{id:(0,0),value:(3,5)}]);
/// ```
pub struct BatchedJoinNode<'a, I, J, L, R, K> {
    state: State<I, J, L, R, K>,
    evaluation: JoinEvaluation,
    left_key: Box<dyn Fn(&L) -> K + 'a>,
    right_key: Box<dyn Fn(&R) -> K + 'a>,
    predicate: Box<Predicate<'a, L, R>>,
    left_pending: SnapshotMap<I, Option<L>>,
    right_pending: SnapshotMap<J, Option<R>>,
    staged: Option<Prepared<I, J, L, R, K>>,
    pending_count: usize,
    preloading: bool,
}
fn update_side<I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &mut JoinSide<I, T, K>,
    changes: &SnapshotMap<I, Option<T>>,
    key: &dyn Fn(&T) -> K,
) {
    let empty = JoinSide::<(), (), K>::new();
    for (id, value) in changes {
        side.apply(
            &empty,
            TupleChange {
                id: id.clone(),
                value: value.clone(),
            },
            key,
            JoinComparison::Equal,
        );
    }
}
fn collect_affected<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>(
    side: &JoinSide<I, L, K>,
    other: &JoinSide<J, R, K>,
    changes: &SnapshotMap<I, Option<L>>,
    mut accept: impl FnMut(I, J),
) {
    for id in changes.keys() {
        if let Some((key, _)) = side.rows.get(id) {
            for other_id in other.matching_ids(key, JoinComparison::Equal) {
                accept(id.clone(), other_id.clone());
            }
        }
    }
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    BatchedJoinNode<'a, I, J, L, R, K>
{
    pub fn new(
        evaluation: JoinEvaluation,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Self {
        Self {
            state: State::new(),
            evaluation,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
            predicate: Box::new(predicate),
            left_pending: SnapshotMap::new(),
            right_pending: SnapshotMap::new(),
            staged: None,
            pending_count: 0,
            preloading: false,
        }
    }
    fn prepare(
        &self,
        state: &State<I, J, L, R, K>,
        left: &SnapshotMap<I, Option<L>>,
        right: &SnapshotMap<J, Option<R>>,
    ) -> Prepared<I, J, L, R, K> {
        let mut next = state.clone();
        update_side(&mut next.left, left, &self.left_key);
        update_side(&mut next.right, right, &self.right_key);
        let mut candidates = SnapshotMap::new();
        for s in [state, &next] {
            collect_affected(&s.left, &s.right, left, |i, j| {
                candidates.insert((i, j), ());
            });
            collect_affected(&s.right, &s.left, right, |j, i| {
                candidates.insert((i, j), ());
            });
        }
        let mut dirty = SnapshotMap::new();
        for (id, _) in &candidates {
            let pair = next
                .left
                .rows
                .get(&id.0)
                .zip(next.right.rows.get(&id.1))
                .filter(|((lk, l), (rk, r))| lk == rk && (self.predicate)(l, r))
                .map(|((_, l), (_, r))| (l.clone(), r.clone()));
            if pair.is_some() || state.pairs.contains_key(id) {
                dirty.insert(id.clone(), ());
            }
            if let Some(pair) = pair {
                next.pairs.insert(id.clone(), pair);
            } else {
                next.pairs.remove(id);
            }
        }
        Prepared { state: next, dirty }
    }
    fn enqueue(&mut self, left: SnapshotMap<I, Option<L>>, right: SnapshotMap<J, Option<R>>) {
        let count = self
            .pending_count
            .checked_add(1)
            .expect("join pending count overflow");
        if self.preloading || self.evaluation == JoinEvaluation::Eager {
            let previous = self.staged.as_ref().map_or(&self.state, |s| &s.state);
            let mut prepared = self.prepare(previous, &left, &right);
            let mut dirty = self
                .staged
                .as_ref()
                .map(|s| s.dirty.clone())
                .unwrap_or_default();
            for id in prepared.dirty.keys() {
                dirty.insert(id.clone(), ());
            }
            prepared.dirty = dirty;
            self.staged = Some(prepared);
        } else {
            let mut lp = self.left_pending.clone();
            let mut rp = self.right_pending.clone();
            for (id, value) in &left {
                lp.insert(id.clone(), value.clone());
            }
            for (id, value) in &right {
                rp.insert(id.clone(), value.clone());
            }
            self.left_pending = lp;
            self.right_pending = rp;
        }
        self.pending_count = count;
    }
    pub fn enqueue_left(&mut self, change: TupleChange<I, L>) {
        let mut left = SnapshotMap::new();
        left.insert(change.id, change.value);
        self.enqueue(left, SnapshotMap::new());
    }
    pub fn enqueue_right(&mut self, change: TupleChange<J, R>) {
        let mut right = SnapshotMap::new();
        right.insert(change.id, change.value);
        self.enqueue(SnapshotMap::new(), right);
    }
    pub fn pending_len(&self) -> usize {
        self.pending_count
    }
    pub fn rows(&self) -> impl Iterator<Item = (&(I, J), &(L, R))> {
        self.state.pairs.iter()
    }
    pub fn discard_pending(&mut self) {
        self.left_pending.clear();
        self.right_pending.clear();
        self.staged = None;
        self.pending_count = 0;
    }
    /// Temporarily use eager evaluation for loading an empty node. Panics if a
    /// preload is already active, committed inputs exist, or a round is pending.
    /// Unmatched inputs count as existing inputs. Propagate before ending preload.
    pub fn begin_preload(&mut self) {
        assert!(
            !self.preloading
                && self.pending_count == 0
                && self.state.left.rows.is_empty()
                && self.state.right.rows.is_empty(),
            "preload requires an empty node without pending work"
        );
        self.preloading = true;
    }
    /// Restore the configured evaluation mode. Panics without an active preload
    /// or while inputs are pending; propagate or discard the round first.
    pub fn end_preload(&mut self) {
        assert!(
            self.preloading && self.pending_count == 0,
            "settle an active preload before ending it"
        );
        self.preloading = false;
    }
    pub fn is_preloading(&self) -> bool {
        self.preloading
    }
    pub fn propagate(&mut self) -> Events<I, J, L, R> {
        if self.pending_count == 0 {
            return Vec::new();
        }
        let prepared = if self.preloading || self.evaluation == JoinEvaluation::Eager {
            self.staged.as_ref().expect("eager pending state").clone()
        } else {
            self.prepare(&self.state, &self.left_pending, &self.right_pending)
        };
        let mut out = Vec::new();
        for id in prepared.dirty.keys() {
            match (
                self.state.pairs.contains_key(id),
                prepared.state.pairs.get(id),
            ) {
                (false, Some(pair)) => out.push(TupleLifecycleEvent::Insert {
                    id: id.clone(),
                    value: pair.clone(),
                }),
                (true, Some(pair)) => out.push(TupleLifecycleEvent::Update {
                    id: id.clone(),
                    value: pair.clone(),
                }),
                (true, None) => out.push(TupleLifecycleEvent::Retract { id: id.clone() }),
                (false, None) => {}
            }
        }
        self.state = prepared.state;
        self.discard_pending();
        out
    }
}

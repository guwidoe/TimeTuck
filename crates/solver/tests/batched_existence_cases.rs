// Adapted from Timefold Community UnindexedIfExistsUniNodeRoundsTest.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
type Node = BatchedExistsNode<'static, usize, usize, i32, i32, ()>;
#[derive(Default)]
struct LiveTracker {
    rows: BTreeMap<usize, i32>,
}
impl LiveTracker {
    fn settle(&mut self, node: &mut Node, left: &[(usize, i32)], right: &[(usize, i32)]) {
        for event in node.propagate() {
            match event {
                TupleLifecycleEvent::Insert { id, value } => assert!(
                    self.rows.insert(id, value).is_none(),
                    "duplicate live insert"
                ),
                TupleLifecycleEvent::Update { id, value } => assert!(
                    self.rows.insert(id, value).is_some(),
                    "update of non-live tuple"
                ),
                TupleLifecycleEvent::Retract { id } => {
                    assert!(self.rows.remove(&id).is_some(), "unbalanced retract")
                }
            }
        }
        let expected = left
            .iter()
            .filter(|(_, l)| right.iter().any(|(_, r)| l > r))
            .copied()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(self.rows, expected);
        assert_eq!(
            self.rows.values().copied().collect::<BTreeSet<_>>(),
            expected.values().copied().collect()
        );
        assert_eq!(
            node.rows()
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
#[test]
fn incremental_rounds_agree_with_brute_force() {
    // Native timing modes replace the two sides of the Java settle-distance
    // threshold. The source explicitly asserts answers and lifecycle balance,
    // not which internal read path ran or predicate-call counts.
    for timing in [ExistenceEvaluation::Eager, ExistenceEvaluation::Deferred] {
        let mut node = Node::new(ExistenceMode::Exists, timing, |_| (), |_| (), |l, r| l > r);
        let mut tracker = LiveTracker::default();
        let mut left = vec![];
        let mut right = vec![];
        for fact in [0, 1, 2, 3, 4] {
            let id = left.len();
            left.push((id, fact));
            node.enqueue_left(TupleChange::put(id, fact));
        }
        for fact in [2, 3] {
            let id = right.len();
            right.push((id, fact));
            node.enqueue_right(TupleChange::put(id, fact));
        }
        tracker.settle(&mut node, &left, &right);
        let reinserted = left.remove(0);
        node.enqueue_left(TupleChange::retract(reinserted.0));
        node.enqueue_left(TupleChange::put(reinserted.0, reinserted.1));
        left.push(reinserted);
        right.push((2, 1));
        node.enqueue_right(TupleChange::put(2, 1));
        tracker.settle(&mut node, &left, &right);
        right[2].1 = 9;
        node.enqueue_right(TupleChange::put(right[2].0, 9));
        tracker.settle(&mut node, &left, &right);
        left[1].1 = 7;
        node.enqueue_left(TupleChange::put(left[1].0, 7));
        tracker.settle(&mut node, &left, &right);
        let removed = right.remove(0);
        node.enqueue_right(TupleChange::retract(removed.0));
        tracker.settle(&mut node, &left, &right);
        for (id, _) in left.drain(..) {
            node.enqueue_left(TupleChange::retract(id));
        }
        tracker.settle(&mut node, &left, &right);
        assert!(tracker.rows.is_empty());
    }
}

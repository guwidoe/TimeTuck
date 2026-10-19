//! Original shared-event ownership and public non-Clone endpoint controls.
use super::*;
use crate::{SimpleScore, StreamBatch};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

type Drops = Rc<RefCell<Vec<i64>>>;
type Calls = Rc<RefCell<Vec<(&'static str, usize, i64, i64)>>>;
struct Row {
    value: i64,
    drops: Drops,
} // Deliberately not Clone.
impl Drop for Row {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.value);
    }
}
type Links = SharedFixedLinks<'static, usize, usize, usize, Row, Row, i64>;
fn node(calls: &Calls, failure: &Rc<Cell<u8>>) -> Links {
    let predicates = calls.clone();
    let projections = calls.clone();
    let predicate_failure = failure.clone();
    let projection_failure = failure.clone();
    SharedFixedLinks {
        links: vec![((1, 2), 3), ((4, 2), 30), ((7, 20), 3)],
        endpoints: vec![(0, 0), (0, 1), (1, 0)],
        by_left: BTreeMap::from([(2, 0), (20, 1)]),
        by_right: BTreeMap::from([(3, 0), (30, 1)]),
        direct_left: false,
        direct_right: false,
        left_links: vec![vec![0, 1], vec![2]],
        right_links: vec![vec![0, 2], vec![1]],
        left: Slots::new(2),
        right: Slots::new(2),
        checkpoint_box: CheckpointBox::new(),
        left_changes: Vec::new(),
        right_changes: Vec::new(),
        affected: Vec::new(),
        left_inputs: vec![usize::MAX; 2],
        right_inputs: vec![usize::MAX; 2],
        inputs_indexed: false,
        predicate: Box::new(move |edge, left, right| {
            predicates
                .borrow_mut()
                .push(("predicate", *edge, left.value, right.value));
            assert!(
                !(predicate_failure.get() == 1 && *edge == 4),
                "predicate panic"
            );
            true
        }),
        projection: Box::new(move |edge, left, right| {
            projections
                .borrow_mut()
                .push(("projection", *edge, left.value, right.value));
            assert!(
                !(projection_failure.get() == 2 && *edge == 4),
                "projection panic"
            );
            *edge as i64 + left.value * 10 + right.value
        }),
    }
}
fn event(id: usize, value: Option<i64>, drops: &Drops) -> Event {
    Rc::new(TupleChange {
        id,
        value: value.map(|value| Row {
            value,
            drops: drops.clone(),
        }),
    })
}
fn initial(drops: &Drops) -> Vec<(bool, Event)> {
    vec![
        (false, event(2, Some(1), drops)),
        (false, event(20, Some(2), drops)),
        (true, event(3, Some(3), drops)),
        (true, event(30, Some(4), drops)),
    ]
}
fn apply(node: &mut Links, inputs: &[(bool, Event)]) -> BatchOutputs {
    <Links as Node<SimpleScore>>::apply_batch(
        node,
        inputs,
        NodeArithmetic {
            impact: crate::ScoringArithmetic::Checked,
            totals: crate::ScoringArithmetic::Checked,
        },
    )
    .unwrap()
}
fn values(outputs: &[Event]) -> Vec<(usize, Option<i64>)> {
    outputs
        .iter()
        .map(|event| {
            let change = typed::<LinkId<usize, usize, usize>, i64>(event);
            (change.id.0.0, change.value)
        })
        .collect()
}

#[test]
fn both_sides_keep_last_actual_event_owners_and_ignore_unlinked_ids() {
    let drops = Drops::default();
    let mut n = node(&Calls::default(), &Rc::new(Cell::new(0)));
    assert_eq!(
        values(&apply(&mut n, &initial(&drops))),
        [(1, Some(14)), (4, Some(18)), (7, Some(30))]
    );
    let left = event(2, Some(5), &drops);
    let right = event(30, Some(7), &drops);
    let unknown = event(99, Some(100), &drops);
    let unknown_owner = Rc::downgrade(&unknown);
    let retraction = event(3, None, &drops);
    let retraction_owner = Rc::downgrade(&retraction);
    let inputs = vec![
        (false, event(2, Some(8), &drops)),
        (false, left.clone()),
        (true, event(3, Some(6), &drops)),
        (true, retraction.clone()),
        (true, right.clone()),
        (false, event(20, None, &drops)),
        (false, unknown.clone()),
        (true, event(usize::MAX, Some(101), &drops)),
    ];
    assert_eq!(
        values(&apply(&mut n, &inputs)),
        [(1, None), (4, Some(61)), (7, None)]
    );
    let left_owner = left.clone().downcast::<TupleChange<usize, Row>>().unwrap();
    let right_owner = right.clone().downcast::<TupleChange<usize, Row>>().unwrap();
    assert!(Rc::ptr_eq(
        n.left.snapshot()[0].as_ref().unwrap(),
        &left_owner
    ));
    assert!(Rc::ptr_eq(
        n.right.snapshot()[1].as_ref().unwrap(),
        &right_owner
    ));
    assert!(n.left.get(1).is_none());
    assert!(n.right.get(0).is_none());
    drop(inputs);
    drop(unknown);
    drop(retraction);
    assert!(unknown_owner.upgrade().is_none());
    assert!(retraction_owner.upgrade().is_none());
    assert!(apply(&mut n, &[]).is_empty());
}

#[test]
fn nested_journals_and_full_snapshot_installs_retain_exact_shared_owners() {
    let drops = Drops::default();
    let mut n = node(&Calls::default(), &Rc::new(Cell::new(0)));
    let left = event(2, Some(1), &drops);
    let right = event(3, Some(3), &drops);
    let left_owner = Rc::downgrade(&left);
    let right_owner = Rc::downgrade(&right);
    drop(apply(&mut n, &[(false, left), (true, right)]));
    let full = <Links as Node<SimpleScore>>::checkpoint(&n);
    let outer = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    drop(apply(&mut n, &[(false, event(2, Some(5), &drops))]));
    let inner = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    drop(apply(
        &mut n,
        &[
            (false, event(2, Some(7), &drops)),
            (true, event(3, Some(6), &drops)),
        ],
    ));
    <Links as Node<SimpleScore>>::commit_checkpoint(&mut n, inner);
    assert!(drops.borrow().is_empty());
    <Links as Node<SimpleScore>>::restore(&mut n, full);
    assert!(drops.borrow().is_empty());
    <Links as Node<SimpleScore>>::restore(&mut n, outer);
    assert_eq!(*drops.borrow(), [7, 5, 6]);
    let original = left_owner.upgrade().unwrap();
    assert!(std::ptr::eq(
        n.left.get(0).unwrap(),
        typed::<usize, Row>(&original)
    ));
    drop(original);
    let held = <Links as Node<SimpleScore>>::checkpoint(&n);
    <Links as Node<SimpleScore>>::clear(&mut n);
    assert!(left_owner.upgrade().is_some());
    assert!(right_owner.upgrade().is_some());
    <Links as Node<SimpleScore>>::restore(&mut n, held);
    assert_eq!(n.left.get(0).unwrap().value.as_ref().unwrap().value, 1);
    <Links as Node<SimpleScore>>::clear(&mut n);
    assert!(left_owner.upgrade().is_none());
    assert!(right_owner.upgrade().is_none());
    assert_eq!(*drops.borrow(), [7, 5, 6, 1, 3]);
}

#[test]
fn predicate_and_projection_panics_leave_shared_rows_recoverable_for_shorter_retry() {
    for failure in [1, 2] {
        let drops = Drops::default();
        let calls = Calls::default();
        let fail = Rc::new(Cell::new(0));
        let mut n = node(&calls, &fail);
        drop(apply(&mut n, &initial(&drops)));
        let saved = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
        let inputs = vec![
            (false, event(2, Some(9), &drops)),
            (true, event(30, Some(8), &drops)),
            (false, event(20, Some(7), &drops)),
            (true, event(3, Some(6), &drops)),
        ];
        fail.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| apply(&mut n, &inputs))).is_err());
        assert_eq!(n.left.get(0).unwrap().value.as_ref().unwrap().value, 1);
        assert_eq!(n.right.get(1).unwrap().value.as_ref().unwrap().value, 4);
        fail.set(0);
        <Links as Node<SimpleScore>>::restore(&mut n, saved);
        drop(inputs);
        let retry = vec![(true, event(3, Some(10), &drops))];
        calls.borrow_mut().clear();
        let actual = values(&apply(&mut n, &retry));
        let observed = calls.borrow().clone();
        let expected_calls = Calls::default();
        let mut fresh = node(&expected_calls, &Rc::new(Cell::new(0)));
        drop(apply(&mut fresh, &initial(&Drops::default())));
        expected_calls.borrow_mut().clear();
        assert_eq!(actual, values(&apply(&mut fresh, &retry)));
        assert_eq!(actual, [(1, Some(21)), (7, Some(37))]);
        assert_eq!(observed, *expected_calls.borrow());
    }
}

#[test]
fn public_non_clone_inputs_outputs_sources_saved_candidates_and_callback_rollback() {
    struct Projected(i64); // Deliberately not Clone too.
    for pruning in [false, true] {
        let drops = Drops::default();
        let fail = Rc::new(Cell::new(false));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let left_source = b.source::<usize, i64>();
        let right_source = b.source::<char, i64>();
        let left_drops = drops.clone();
        let left = b
            .filter_map_batched(left_source.port(), move |v| {
                (*v >= 0).then(|| Row {
                    value: *v,
                    drops: left_drops.clone(),
                })
            })
            .unwrap();
        let right_drops = drops.clone();
        let right = b
            .filter_map_batched(right_source.port(), move |v| {
                (*v >= 0).then(|| Row {
                    value: *v,
                    drops: right_drops.clone(),
                })
            })
            .unwrap();
        let failure = fail.clone();
        let joined = b
            .join_fixed_links_batched_shared(
                &left,
                &right,
                [(9_i64, 20, 'z'), (1, 2, 'a'), (4, 2, 'z'), (7, 20, 'a')],
                |edge, a, c| (*edge + a.value + c.value) % 3 != 0,
                move |edge, a, c| {
                    assert!(!failure.get() || *edge != 4, "projection panic");
                    Projected(*edge + a.value * 10 + c.value)
                },
            )
            .unwrap();
        let numbers = b.filter_map(&joined, |value| Some(value.0)).unwrap();
        let data = b.dataset(&numbers).unwrap();
        b.score("links", &numbers, |&value| SimpleScore::new(value))
            .unwrap();
        let mut g = b.build();
        let mut initial = StreamBatch::new();
        initial.push(&left_source, TupleChange::put(2, 1)).unwrap();
        initial.push(&left_source, TupleChange::put(20, 2)).unwrap();
        initial
            .push(&right_source, TupleChange::put('a', 3))
            .unwrap();
        initial
            .push(&right_source, TupleChange::put('z', 4))
            .unwrap();
        let mut tx = g.transaction();
        tx.apply_batch(initial).unwrap();
        tx.commit().unwrap();
        let held = g.dataset_rows(&data).unwrap().clone();
        assert_eq!(g.score(), SimpleScore::new(14));
        let mut update = StreamBatch::new();
        update.push(&left_source, TupleChange::put(2, 9)).unwrap();
        update.push(&left_source, TupleChange::put(2, 5)).unwrap();
        update.push(&left_source, TupleChange::put(20, -1)).unwrap();
        update
            .push(&left_source, TupleChange::put(99, 100))
            .unwrap();
        update
            .push(&right_source, TupleChange::put('a', 9))
            .unwrap();
        update
            .push(&right_source, TupleChange::retract('a'))
            .unwrap();
        update
            .push(&right_source, TupleChange::put('z', 7))
            .unwrap();
        fail.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply_batch(update.clone()).unwrap();
            }))
            .is_err()
        );
        fail.set(false);
        assert_eq!(g.score(), SimpleScore::new(14));
        assert_eq!(g.dataset_rows(&data).unwrap(), &held);
        let saved = {
            let mut tx = g.transaction();
            tx.apply_batch(update).unwrap();
            tx.assert_consistent().unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(61));
            tx.capture().unwrap()
        };
        assert_eq!(g.score(), SimpleScore::new(14));
        assert_eq!(g.dataset_rows(&data).unwrap(), &held);
        g.install(saved);
        assert_eq!(g.score(), SimpleScore::new(61));
        assert_eq!(g.source_rows(&left_source).unwrap().get(&99), Some(&100));
        let actual: Vec<_> = g
            .dataset_rows(&data)
            .unwrap()
            .iter()
            .map(|(id, value)| (*id, *value))
            .collect();
        assert_eq!(actual, [(((4, 2), 'z'), 61)]);
        g.assert_consistent().unwrap();
        assert_eq!(held.len(), 1);
        assert_eq!(held[&((1, 2), 'a')], 14);
    }
}

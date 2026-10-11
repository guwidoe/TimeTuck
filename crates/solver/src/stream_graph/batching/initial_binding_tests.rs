//! Original constructor-path regressions; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
};
#[derive(Clone, Debug)]
struct Key(usize, Rc<Cell<usize>>);
impl PartialEq for Key {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Key {
    fn cmp(&self, r: &Self) -> Ordering {
        self.1.set(self.1.get() + 1);
        self.0.cmp(&r.0)
    }
}
#[test]
fn unused_sorted_rows_build_linearly_and_reactivate_through_original_handles() {
    for fast in [false, true] {
        let comparisons = Rc::new(Cell::new(0));
        let order = RefCell::new(Vec::new());
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let unused = b.source::<Key, i64>();
        let observed = b.source::<usize, i64>();
        let terminal = b
            .score_weighted("dormant", unused.port(), SimpleScore::ZERO, |v| *v)
            .unwrap();
        b.score("seen", observed.port(), |v| {
            order.borrow_mut().push(*v);
            SimpleScore::new(*v)
        })
        .unwrap();
        let mut g = b.build();
        let mut batch = StreamBatch::new();
        batch.push(&observed, TupleChange::put(0, 7)).unwrap();
        for i in 0..1024 {
            batch
                .push(
                    &unused,
                    TupleChange::put(Key(i, comparisons.clone()), i as i64 + 1),
                )
                .unwrap();
        }
        batch.push(&observed, TupleChange::put(0, 9)).unwrap();
        let mut tx = g.transaction();
        if fast {
            tx.bind_planning_rows(batch).unwrap();
        } else {
            tx.apply_updates(batch).unwrap();
        }
        let count = comparisons.get();
        assert_eq!(*order.borrow(), vec![7, 9]);
        assert_eq!(tx.score_evaluations(), 2);
        assert_eq!(tx.checkpoint_count(), 3);
        assert_eq!(
            tx.graph().unwrap().source_rows(&unused).unwrap().len(),
            1024
        );
        if fast {
            assert_eq!(count, 1023);
        } else {
            assert!(count > 8000);
        }
        tx.commit().unwrap();
        assert_eq!(
            g.source_rows(&unused)
                .unwrap()
                .iter()
                .map(|(k, v)| (k.0, *v))
                .collect::<Vec<_>>(),
            (0..1024).map(|i| (i, i as i64 + 1)).collect::<Vec<_>>()
        );
        let work = g
            .apply_weight_overrides(
                &[("dormant".to_owned(), SimpleScore::new(2))]
                    .into_iter()
                    .collect(),
            )
            .unwrap();
        assert_eq!(work, 1025);
        assert_eq!(g.constraint_matches(&terminal).unwrap().len(), 1024);
        assert_eq!(g.score(), SimpleScore::new(9 + 2 * 1024 * 1025 / 2));
        g.assert_consistent().unwrap();
    }
}
#[test]
fn small_unsorted_duplicate_nonempty_observed_and_activity_paths_keep_order() {
    for mode in 0..7 {
        let calls = RefCell::new(Vec::new());
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if mode == 5 {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        if mode == 4 {
            b.score("seen", source.port(), |v| {
                calls.borrow_mut().push(*v);
                SimpleScore::new(*v)
            })
            .unwrap();
        }
        let mut g = b.build();
        if mode == 6 {
            g.batched_updates = true;
        }
        if mode == 3 {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(9999, 7)).unwrap();
            tx.commit().unwrap();
        }
        let mut changes: Vec<_> = (0..if mode == 0 { 8 } else { 512 })
            .map(|i| TupleChange::put(i, i as i64))
            .collect();
        if mode == 1 {
            changes.reverse();
        }
        if mode == 2 {
            changes.push(TupleChange::put(0, 1000));
            changes.push(TupleChange::retract(1));
        }
        let mut batch = StreamBatch::new();
        for c in &changes {
            batch.push(&source, c.clone()).unwrap();
        }
        let mut tx = g.transaction();
        tx.bind_planning_rows(batch).unwrap();
        let rows = tx.graph().unwrap().source_rows(&source).unwrap();
        let mut expected = std::collections::BTreeMap::new();
        if mode == 3 {
            expected.insert(9999, 7);
        }
        for c in &changes {
            if let Some(v) = c.value {
                expected.insert(c.id, v);
            } else {
                expected.remove(&c.id);
            }
        }
        assert_eq!(rows, &expected);
        if mode == 4 {
            assert_eq!(
                *calls.borrow(),
                changes.iter().filter_map(|c| c.value).collect::<Vec<_>>()
            );
        }
        drop(tx);
        assert_eq!(
            g.source_rows(&source).unwrap().len(),
            usize::from(mode == 3)
        );
        g.assert_consistent().unwrap();
    }
}
#[derive(Debug)]
struct Payload(usize, Rc<Cell<Option<usize>>>);
impl Clone for Payload {
    fn clone(&self) -> Self {
        if let Some(left) = self.1.get() {
            assert_ne!(left, 0, "source clone panic");
            self.1.set(Some(left - 1));
        }
        Self(self.0, self.1.clone())
    }
}
#[test]
fn partial_clone_panic_and_later_callback_error_restore_all_sources() {
    for failure in [Some(0), Some(1), Some(127), Some(255), Some(511), None] {
        let bomb = Rc::new(Cell::new(None));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let unused = b.source::<usize, Payload>();
        let observed = b.source::<usize, i64>();
        b.score("late", observed.port(), |v| {
            SimpleScore::new(*v).with_structural(i64::from(*v == 2))
        })
        .unwrap();
        let mut g = b.build();
        let mut batch = StreamBatch::new();
        batch.push(&observed, TupleChange::put(0, 1)).unwrap();
        for i in 0..512 {
            batch
                .push(&unused, TupleChange::put(i, Payload(i, bomb.clone())))
                .unwrap();
        }
        batch.push(&observed, TupleChange::put(0, 2)).unwrap();
        bomb.set(failure);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.bind_planning_rows(batch)
        }));
        bomb.set(None);
        if failure.is_some() {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(GraphError::StructuralContribution(_))
            ));
        }
        assert!(g.source_rows(&unused).unwrap().is_empty());
        assert!(g.source_rows(&observed).unwrap().is_empty());
        assert_eq!(g.score(), SimpleScore::ZERO);
        g.assert_consistent().unwrap();
    }
}

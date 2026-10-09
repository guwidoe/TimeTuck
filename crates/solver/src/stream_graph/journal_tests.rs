//! Original graph journal interactions; no upstream-method credit.
use super::*;
use crate::{SimpleScore, StreamBatch};
use std::cell::Cell;

#[test]
fn journal_trials_mix_replacements_structural_changes_and_saved_candidates() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let positive = b.filter(source.port(), |v| *v > 0).unwrap();
        let dataset = b.dataset(&positive).unwrap();
        let terminal = b
            .score("positive", &positive, |v| SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            for (id, value) in [(0, 1), (1, 2), (2, -1)] {
                tx.apply(&source, TupleChange::put(id, value)).unwrap();
            }
            tx.commit().unwrap();
        }
        let before = g.source_rows(&source).unwrap().clone();
        let old_matches = g.constraint_matches(&terminal).unwrap().clone();
        let old_dataset = g.dataset_rows(&dataset).unwrap().clone();
        let saved;
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 5)).unwrap();
            tx.apply(&source, TupleChange::put(2, -2)).unwrap(); // Rejected row also replaces.
            tx.assert_consistent().unwrap();
            let mut batch = StreamBatch::new();
            batch.push(&source, TupleChange::put(1, -5)).unwrap();
            batch.push(&source, TupleChange::put(2, 8)).unwrap();
            batch.push(&source, TupleChange::put(3, 10)).unwrap();
            tx.apply_batch(batch).unwrap();
            tx.apply(&source, TupleChange::retract(0)).unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(18));
            tx.assert_consistent().unwrap();
            saved = tx.capture().unwrap();
        }
        assert_eq!(g.source_rows(&source).unwrap(), &before);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &old_matches);
        assert_eq!(g.dataset_rows(&dataset).unwrap(), &old_dataset);
        assert_eq!(g.score(), SimpleScore::new(3));
        g.install(saved);
        assert_eq!(g.score(), SimpleScore::new(18));
        g.assert_consistent().unwrap();
        for i in 0..16 {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(2, i)).unwrap();
            tx.apply(&source, TupleChange::put(3, 100 - i)).unwrap();
            tx.assert_consistent().unwrap();
            if i % 2 == 0 {
                tx.commit().unwrap();
            }
        }
        g.assert_consistent().unwrap();
        assert_eq!(before[&0], 1);
        assert_eq!(old_dataset[&1], 2);
        assert_eq!(old_matches[&0].1, SimpleScore::new(1));
    }
}

#[test]
fn journal_diagnostics_detect_cancelling_scores_and_dataset_changes_and_restore() {
    let corrupt = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let filtered = b.filter(source.port(), |_| true).unwrap();
    let terminal = b
        .score("sum", &filtered, |v| {
            SimpleScore::new(if corrupt.get() { 3 - v } else { *v })
        })
        .unwrap();
    let mapped = b
        .map(&filtered, |v| if corrupt.get() { -*v } else { *v })
        .unwrap();
    let dataset = b.dataset(&mapped).unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 1)).unwrap();
        tx.apply(&source, TupleChange::put(1, 2)).unwrap();
        tx.commit().unwrap();
    }
    corrupt.set(true);
    let report = g.consistency_report().unwrap();
    assert_eq!(report.working_score, report.recomputed_score);
    assert_eq!(report.nodes.len(), 2);
    assert_eq!(
        g.constraint_matches(&terminal).unwrap()[&0].1,
        SimpleScore::new(1)
    );
    assert_eq!(g.dataset_rows(&dataset).unwrap()[&0], 1);
    corrupt.set(false);
    g.assert_consistent().unwrap();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(7));
}

#[test]
fn transaction_buffers_survive_nested_rebuilds_failures_and_saved_candidates() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for pruning in [false, true] {
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let filtered = b
            .filter(source.port(), |v| {
                assert!(!fail.get());
                *v > 0
            })
            .unwrap();
        let groups = b
            .group_collect_batched(&filtered, |v| (0usize, *v))
            .unwrap();
        b.score("sum", &groups, |(_, s)| SimpleScore::new(s.sum))
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 1)).unwrap();
            tx.commit().unwrap();
        }
        let visited_pointer = g.transaction_workspaces.last().unwrap().visited.as_ptr();
        let journal_pointer = g.transaction_workspaces.last().unwrap().journal.as_ptr();
        for n in 2..130 {
            let before = g.score();
            let saved;
            {
                let mut tx = g.transaction();
                let mut batch = StreamBatch::new();
                batch.push(&source, TupleChange::put(0, n)).unwrap();
                batch.push(&source, TupleChange::put(0, n + 1)).unwrap();
                tx.apply_batch(batch).unwrap();
                tx.assert_consistent().unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::new(n + 1));
                saved = tx.capture().unwrap();
                if n % 2 == 0 {
                    tx.commit().unwrap();
                }
            }
            assert_eq!(
                g.score(),
                if n % 2 == 0 {
                    SimpleScore::new(n + 1)
                } else {
                    before
                }
            );
            assert_eq!(g.transaction_workspaces.len(), 2);
            let top = g.transaction_workspaces.last().unwrap();
            assert_eq!(top.visited.as_ptr(), visited_pointer);
            assert_eq!(top.journal.as_ptr(), journal_pointer);
            assert!(
                g.transaction_workspaces
                    .iter()
                    .all(|w| w.journal.is_empty() && w.visited.iter().all(|&v| !v))
            );
            g.install(saved);
            assert_eq!(g.score(), SimpleScore::new(n + 1));
        }
        let before = g.score();
        fail.set(true);
        {
            let mut tx = g.transaction();
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || tx.apply(&source, TupleChange::put(0, 999))
                ))
                .is_err()
            );
            assert!(tx.score().is_err());
        }
        fail.set(false);
        assert_eq!(g.score(), before);
        g.assert_consistent().unwrap();
        assert_eq!(g.transaction_workspaces.len(), 2);
        assert!(
            g.transaction_workspaces
                .iter()
                .all(|w| w.journal.is_empty() && w.visited.iter().all(|&v| !v))
        );
    }
}

#[test]
fn recycled_transaction_buffers_release_candidate_and_committed_payloads() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, Rc<()>>();
    b.score_fixed("one", source.port(), [0], |_| {
        assert!(!fail.get());
        SimpleScore::new(1)
    })
    .unwrap();
    let mut g = b.build();
    for mode in 0..3 {
        let payload = Rc::new(());
        let weak = Rc::downgrade(&payload);
        {
            let mut tx = g.transaction();
            if mode == 2 {
                fail.set(true);
                assert!(
                    catch_unwind(AssertUnwindSafe(
                        || tx.apply(&source, TupleChange::put(0, payload))
                    ))
                    .is_err()
                );
            } else {
                tx.apply(&source, TupleChange::put(0, payload)).unwrap();
                if mode == 1 {
                    tx.commit().unwrap();
                }
            }
        }
        fail.set(false);
        if mode == 1 {
            assert!(weak.upgrade().is_some());
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::retract(0)).unwrap();
            tx.commit().unwrap();
        }
        assert!(
            weak.upgrade().is_none(),
            "mode {mode}: spare buffers retain payloads"
        );
        assert!(
            g.transaction_workspaces
                .iter()
                .all(|w| w.journal.is_empty() && w.visited.iter().all(|&v| !v))
        );
        assert_eq!(g.score(), SimpleScore::ZERO);
    }
}

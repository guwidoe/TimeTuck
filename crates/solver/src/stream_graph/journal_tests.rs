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

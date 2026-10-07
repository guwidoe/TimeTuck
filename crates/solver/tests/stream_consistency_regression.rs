//! Original retained-state corruption evidence and recovery regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn report_detects_cancelling_constraints_matches_and_dataset_payloads() {
    let factor = Cell::new(1i64);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let balanced = b.source::<usize, i64>();
    for (name, sign) in [("gain", 1), ("loss", -1)] {
        let factor = &factor;
        let calls = &calls;
        b.score(name, s.port(), move |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(sign * v * factor.get())
        })
        .unwrap();
    }
    let terminal = b
        .score("balanced", balanced.port(), |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v * factor.get())
        })
        .unwrap();
    let mapped = b.map(s.port(), |v| v * factor.get()).unwrap();
    let dataset = b.dataset(&mapped).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 3)).unwrap();
    tx.apply(&balanced, TupleChange::put(0, 1)).unwrap();
    tx.apply(&balanced, TupleChange::put(1, -1)).unwrap();
    tx.commit().unwrap();
    let old = g.constraint_matches(&terminal).unwrap().clone();
    assert!(g.consistency_report().unwrap().is_consistent());
    factor.set(2);
    calls.set(0);
    let report = g.consistency_report().unwrap();
    assert_eq!(calls.get(), 4);
    assert_eq!(report.working_score, SimpleScore::ZERO);
    assert_eq!(report.recomputed_score, SimpleScore::ZERO);
    assert!(!report.is_consistent());
    assert_eq!(report.nodes.len(), 4);
    let changed = |name| {
        report
            .nodes
            .iter()
            .find(|n| n.constraint.as_deref() == Some(name))
            .unwrap()
    };
    assert_eq!(changed("gain").working_score, SimpleScore::new(3));
    assert_eq!(changed("gain").recomputed_score, SimpleScore::new(6));
    assert_eq!(changed("loss").recomputed_score, SimpleScore::new(-6));
    assert_eq!(
        changed("balanced").working_score,
        changed("balanced").recomputed_score
    );
    assert_eq!(
        report
            .nodes
            .iter()
            .filter(|n| n.constraint.is_none())
            .count(),
        1
    );
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &old);
    assert_eq!(g.dataset_rows(&dataset).unwrap()[&0], 3);
    assert_eq!(g.score(), SimpleScore::ZERO);
    let text = report.diagnostic().to_string();
    assert!(text.contains("balanced") && text.contains("dataset node"));
    factor.set(1);
    assert!(g.consistency_report().unwrap().is_consistent());
    assert!(!report.is_consistent()); // Detached evidence survives later checks.
}
#[test]
fn error_keeps_all_nodes_and_poisoned_trials_roll_back_without_counting_diagnostics() {
    let factor = Cell::new(1i64);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    for i in 0..8 {
        let factor = &factor;
        b.score(format!("c{i}"), s.port(), move |v| {
            SimpleScore::new(v * factor.get())
        })
        .unwrap();
    }
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 4)).unwrap();
    let evaluations = tx.score_evaluations();
    factor.set(2);
    let error = tx.assert_consistent().unwrap_err();
    assert_eq!(tx.score_evaluations(), evaluations);
    let GraphError::CorruptedScore(report) = &error else {
        panic!("{error}")
    };
    assert_eq!(report.nodes.len(), 8);
    assert!(report.to_string().contains("3 more node differences"));
    assert!(report.to_string().contains("c4"));
    assert!(!report.to_string().contains("c5"));
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(g.score(), SimpleScore::new(24));
    assert_eq!(g.source_rows(&s).unwrap()[&0], 3);
    factor.set(1);
    assert!(g.consistency_report().unwrap().is_consistent());
}
#[test]
fn report_restores_on_callback_panics_and_preserves_arithmetic_errors() {
    let panic = Cell::new(false);
    let factor = Cell::new(1i64);
    let mut b =
        StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(ScoringArithmetic::Checked);
    let s = b.source::<usize, i64>();
    let c = b
        .score_weighted("checked", s.port(), SimpleScore::new(i64::MAX), |_| {
            assert!(!panic.get(), "injected panic");
            factor.get()
        })
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    let old = g.constraint_matches(&c).unwrap().clone();
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| g.consistency_report())).is_err());
    panic.set(false);
    assert_eq!(g.constraint_matches(&c).unwrap(), &old);
    factor.set(2);
    assert_eq!(
        g.consistency_report(),
        Err(GraphError::ScoreOverflow {
            constraint: Some("checked".into())
        })
    );
    assert_eq!(g.constraint_matches(&c).unwrap(), &old);
    assert_eq!(g.score(), SimpleScore::new(i64::MAX));
    factor.set(1);
    assert!(g.consistency_report().unwrap().is_consistent());
}

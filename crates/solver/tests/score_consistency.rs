//! Original structured corruption diagnostics and candidate rollback regressions.
use std::cell::Cell;
use timetuck::*;
#[test]
fn reports_all_changes_once_with_structural_scores_without_repairing_caches() {
    let value = Cell::new(0);
    let calls = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    for i in 0..8 {
        let value = &value;
        let calls = &calls;
        n.add_constraint(format!("c{i}"), vec![], move |_| {
            calls.set(calls.get() + 1);
            SimpleScore::new(if i % 2 == 0 {
                value.get()
            } else {
                -value.get()
            })
        })
        .unwrap();
    }
    let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    value.set(5);
    calls.set(0);
    let report = d.consistency_report().unwrap();
    assert_eq!(calls.get(), 8);
    assert!(!report.is_consistent());
    assert_eq!(report.working_score, SimpleScore::ZERO.with_structural(-1));
    assert_eq!(report.recomputed_score, report.working_score);
    assert_eq!(report.constraints.len(), 8);
    assert_eq!(
        report.constraints[0],
        ConstraintScoreDifference {
            constraint: 0,
            name: "c0".into(),
            working_score: SimpleScore::ZERO,
            recomputed_score: SimpleScore::new(5)
        }
    );
    assert_eq!(d.score(), report.working_score);
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert!(d.constraint_scores().all(|(_, s)| s == SimpleScore::ZERO));
    let text = report.diagnostic().to_string();
    assert!(text.contains("Score corruption"));
    assert!(text.contains("3 more constraint differences"));
    assert!(text.contains("c4"));
    assert!(!text.contains("c5"));
    value.set(0);
    assert!(d.consistency_report().unwrap().is_consistent());
}
#[test]
fn failed_recomputation_keeps_its_arithmetic_error_and_original_cache() {
    let value = Cell::new(0);
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let mut n = ConstraintNetwork::new(0);
    for i in 0..2 {
        n.add_constraint(format!("c{i}"), vec![], |_| SimpleScore::new(value.get()))
            .unwrap();
    }
    let d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    value.set(i64::MAX);
    assert_eq!(
        d.consistency_report(),
        Err(ScoringError::ScoreOverflow { constraint: None })
    );
    assert_eq!(d.score(), SimpleScore::ZERO);
    value.set(1);
    assert_eq!(
        d.consistency_report().unwrap().recomputed_score,
        SimpleScore::new(2)
    );
}
#[test]
fn typed_equality_does_not_use_score_debug_text() {
    #[derive(Clone, PartialEq, Eq)]
    struct SameDebug(u8);
    impl std::fmt::Debug for SameDebug {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("identical label")
        }
    }
    let report = ScoreConsistencyReport {
        working_score: SameDebug(1),
        recomputed_score: SameDebug(2),
        constraints: vec![],
    };
    assert!(!report.is_consistent());
    assert_eq!(
        report.diagnostic().working_score,
        report.diagnostic().recomputed_score
    );
}

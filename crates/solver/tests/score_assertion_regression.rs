//! Original score assertion integration regressions.
use std::cell::Cell;
use timetuck::*;

#[test]
fn scoped_snapshot_preserves_structural_score_and_complete_failure_without_rescoring() {
    let calls = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    for i in 0..8 {
        let calls = &calls;
        n.add_constraint(format!("c{i}"), vec![], move |_| {
            calls.set(calls.get() + 1);
            SimpleScore::new(-1)
        })
        .unwrap();
    }
    let director = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let count = calls.get();
    let a = ScoreAssertion::from_scores(director.score(), director.constraint_scores()).unwrap();
    a.check(
        ConstraintComparison::Exactly(SimpleScore::new(-8).with_structural(-1)),
        None,
    )
    .unwrap();
    a.check(ConstraintComparison::LessThan(SimpleScore::new(-100)), None)
        .unwrap();
    let error = a
        .check(
            ConstraintComparison::Exactly(SimpleScore::new(-8)),
            Some("assignment incomplete"),
        )
        .unwrap_err();
    assert_eq!(calls.get(), count);
    assert_eq!(error.actual, director.score());
    assert_eq!(error.constraints.len(), 8);
    assert!(error.to_string().contains("assignment incomplete"));
    assert!(error.to_string().contains("3 more constraints"));
    assert!(error.to_string().contains("c4"));
    assert!(!error.to_string().contains("c5"));
}

#[test]
fn graph_snapshot_is_detached_includes_disabled_constraints_and_never_rescores() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    b.score_weighted("live", source.port(), SimpleScore::new(2), |v| {
        calls.set(calls.get() + 1);
        *v
    })
    .unwrap();
    b.score_weighted("off", source.port(), SimpleScore::ZERO, |_| {
        panic!("disabled")
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    let before = score_assertion(&graph);
    assert_eq!(calls.get(), 1);
    assert_eq!(before.constraints()["off"], SimpleScore::ZERO);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    let after = score_assertion(&graph);
    assert_eq!(calls.get(), 2);
    assert_eq!(before.score(), &SimpleScore::new(6));
    assert_eq!(after.score(), &SimpleScore::new(10));
    assert_eq!(before.constraints()["live"], SimpleScore::new(6));
    assert!(
        before
            .check(ConstraintComparison::MoreThan(SimpleScore::new(6)), None)
            .is_err()
    );
    assert!(
        before
            .check(ConstraintComparison::LessThan(SimpleScore::new(6)), None)
            .is_err()
    );
}

#[test]
fn comparisons_use_typed_values_and_duplicate_names_are_rejected() {
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Opaque(i64);
    impl std::fmt::Debug for Opaque {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("same text")
        }
    }
    let a = ScoreAssertion::from_scores(Opaque(1), [("a", Opaque(1))]).unwrap();
    assert!(
        a.check(ConstraintComparison::Exactly(Opaque(2)), None)
            .is_err()
    );
    a.check(ConstraintComparison::LessThan(Opaque(2)), None)
        .unwrap();
    assert_eq!(
        ScoreAssertion::from_scores(0, [("duplicate", 1), ("duplicate", 2)]),
        Err(GraphError::DuplicateConstraint("duplicate".into()))
    );
    let empty = score_assertion(&StreamGraphBuilder::<SimpleScore>::new().build());
    empty
        .check(ConstraintComparison::Exactly(SimpleScore::ZERO), None)
        .unwrap();
    assert!(empty.constraints().is_empty());
}

#[test]
fn decimal_scores_retain_all_digits_and_compare_numeric_values() {
    let score: SimpleDecimalScore = "9007199254740993.001".parse().unwrap();
    let a = ScoreAssertion::from_scores(score.clone(), [("fraction", score.clone())]).unwrap();
    a.check(ConstraintComparison::Exactly(score), None).unwrap();
    a.check(
        ConstraintComparison::MoreThan("9007199254740993.000".parse().unwrap()),
        None,
    )
    .unwrap();
    let different_scale: SimpleDecimalScore = "9007199254740993.0010".parse().unwrap();
    // Decimal scores compare numerically, including across different scales.
    assert_eq!(a.score(), &different_scale);
    a.check(ConstraintComparison::Exactly(different_scale), None)
        .unwrap();
}

//! Original executable configuration controls; no upstream method credit.
#[path = "fixtures/custom_justification_fixture.rs"]
mod fixture;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
struct Justification {
    score: SimpleScore,
    code: String,
}
#[test]
fn configured_callback_preserves_first_registration_and_uses_live_retained_scores() {
    let score_calls = Cell::new(0);
    let justify_calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (String, i64)>();
    let mut declaration = CustomConstraintDeclaration::new(
        source.port(),
        "cost",
        SimpleScore::new(-1),
        |(_, amount)| {
            score_calls.set(score_calls.get() + 1);
            *amount
        },
    );
    declaration
        .justify_with(|(code, _), score| {
            justify_calls.set(justify_calls.get() + 1);
            Justification {
                score: *score,
                code: code.clone(),
            }
        })
        .unwrap();
    assert_eq!(
        declaration.justify_with(|_, _| panic!("duplicate cannot replace first callback")),
        Err(ConstraintDeclarationError::MultipleJustifications)
    );
    let declared = declaration.build(&mut b).unwrap();
    let mut weights = ConstraintWeightOverrides::default();
    weights.insert("cost", SimpleScore::new(-3));
    let mut graph = b.build_with_weights(&weights).unwrap();
    assert_eq!(score_calls.get(), 0);
    assert_eq!(justify_calls.get(), 0);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(1, ("one".into(), 2)))
        .unwrap();
    tx.apply(&source, TupleChange::put(2, ("zero".into(), 0)))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-6));
    assert_eq!(score_calls.get(), 2);
    let report = declared
        .explain(&graph, |(code, _)| vec![code.clone(), code.clone()])
        .unwrap();
    assert_eq!(report.score, SimpleScore::new(-6));
    assert_eq!(report.matches.len(), 2);
    assert_eq!(
        report.matches[&1].justification,
        Justification {
            score: SimpleScore::new(-6),
            code: "one".into()
        }
    );
    assert_eq!(report.matches[&2].justification.score, SimpleScore::ZERO);
    assert_eq!(report.indictments.len(), 2);
    assert_eq!(score_calls.get(), 2);
    assert_eq!(justify_calls.get(), 2);
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(1, ("replacement".into(), 5)))
            .unwrap();
    }
    let report = declared
        .explain(&graph, |(code, _)| vec![code.clone()])
        .unwrap();
    assert_eq!(report.matches[&1].justification.code, "one");
    assert_eq!(report.score, SimpleScore::new(-6));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(2)).unwrap();
    tx.apply(&source, TupleChange::put(1, ("replacement".into(), 5)))
        .unwrap();
    tx.commit().unwrap();
    let before = score_calls.get();
    let report = declared
        .explain(&graph, |(code, _)| vec![code.clone()])
        .unwrap();
    assert_eq!(report.score, SimpleScore::new(-15));
    assert_eq!(report.matches[&1].justification.code, "replacement");
    assert!(!report.indictments.contains_key("one"));
    assert_eq!(score_calls.get(), before);
    graph.assert_consistent().unwrap();
}
#[test]
fn missing_foreign_and_panicking_explanation_paths_leave_scoring_intact() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let missing = CustomConstraintDeclaration::<_, _, _, Justification>::new(
        source.port(),
        "missing",
        SimpleScore::new(1),
        |v| *v,
    );
    assert!(matches!(
        missing.build(&mut b),
        Err(ConstraintDeclarationError::MissingJustification)
    ));
    let mut declaration =
        CustomConstraintDeclaration::new(source.port(), "valid", SimpleScore::new(1), |v| *v);
    declaration
        .justify_with(|_, _| -> Justification { panic!("controlled explanation failure") })
        .unwrap();
    let configured = declaration.build(&mut b).unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 4)).unwrap();
    tx.commit().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| configured
            .explain(&graph, |v| vec![*v])
            .unwrap()))
        .is_err()
    );
    assert_eq!(graph.score(), SimpleScore::new(4));
    graph.assert_consistent().unwrap();
    let foreign = StreamGraphBuilder::<SimpleScore>::new().build();
    assert_eq!(
        configured.explain(&foreign, |v| vec![*v]).unwrap_err(),
        GraphError::ForeignPort
    );
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let mut declaration =
        CustomConstraintDeclaration::new(source.port(), "foreign", SimpleScore::new(1), |v| *v);
    declaration
        .justify_with(|v, score| Justification {
            score: *score,
            code: v.to_string(),
        })
        .unwrap();
    assert!(matches!(
        declaration.build(&mut other),
        Err(ConstraintDeclarationError::Graph(GraphError::ForeignPort))
    ));
}
#[test]
fn ordered_conditions_execute_indexes_and_filters_with_incremental_undo() {
    let first = Cell::new(0);
    let second = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, (i64, i64)>();
    let right = b.source::<usize, (i64, i64)>();
    let lk = JoinProjection::new(|v: &(i64, i64)| v.0);
    let rk = JoinProjection::new(|v: &(i64, i64)| v.0);
    let conditions = OrderedJoinConditions::new()
        .indexed(JoinCondition::new(JoinComparison::Equal, &lk, &rk))
        .filtering(|l, r| {
            first.set(first.get() + 1);
            l.1 < r.1
        })
        .filtering(|l, r| {
            second.set(second.get() + 1);
            l.1 + r.1 > 0
        });
    let joined = conditions.build(&mut b, left.port(), right.port()).unwrap();
    let constraint = b.score("pairs", &joined, |_| SimpleScore::new(-1)).unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&left, TupleChange::put(0, (1, -5))).unwrap();
    tx.apply(&left, TupleChange::put(1, (1, 3))).unwrap();
    tx.apply(&right, TupleChange::put(0, (1, 2))).unwrap();
    tx.apply(&right, TupleChange::put(1, (2, 100))).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::ZERO);
    assert_eq!(first.get(), 2);
    assert_eq!(second.get(), 1);
    {
        let mut tx = graph.transaction();
        tx.apply(&right, TupleChange::put(0, (1, 10))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(-2));
    }
    assert_eq!(graph.score(), SimpleScore::ZERO);
    let mut tx = graph.transaction();
    tx.apply(&right, TupleChange::put(0, (1, 10))).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-2));
    assert_eq!(graph.constraint_matches(&constraint).unwrap().len(), 2);
    graph.assert_consistent().unwrap();
}
#[test]
fn invalid_condition_order_never_evaluates_callbacks_or_adds_join_state() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, i64>();
    let right = b.source::<usize, i64>();
    let lk = JoinProjection::new(|_: &i64| -> i64 { panic!("invalid index callback") });
    let rk = lk.clone();
    let conditions = OrderedJoinConditions::new()
        .filtering(|_, _| panic!("invalid filter callback"))
        .indexed(JoinCondition::new(JoinComparison::Equal, &lk, &rk));
    assert!(matches!(
        conditions.build(&mut b, left.port(), right.port()),
        Err(OrderedJoinError::IndexedAfterFiltering)
    ));
    b.score("left", left.port(), |v| SimpleScore::new(*v))
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&left, TupleChange::put(0, 3)).unwrap();
    tx.apply(&right, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(3));
    graph.assert_consistent().unwrap();
}

#[test]
fn attributed_justification_fixture_retains_the_actual_ordered_objects() {
    use std::sync::Arc;
    let a = Arc::new(String::from("a"));
    let b_object = Arc::new(String::from("b"));
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (Arc<String>, Arc<String>)>();
    let mut declaration =
        CustomConstraintDeclaration::new(source.port(), "objects", SimpleScore::new(-1), |_| 2);
    declaration
        .justify_with(|(a, b), score| fixture::TestJustification {
            score: *score,
            facts: vec![a.clone(), b.clone(), a.clone()],
        })
        .unwrap();
    let configured = declaration.build(&mut builder).unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (a.clone(), b_object.clone())))
        .unwrap();
    tx.commit().unwrap();
    let report = configured
        .explain(&graph, |(a, b)| {
            vec![a.to_string(), b.to_string(), a.to_string()]
        })
        .unwrap();
    let actual = &report.matches[&0].justification;
    assert_eq!(actual.score, SimpleScore::new(-2));
    assert_eq!(actual.facts.len(), 3);
    for (fact, expected) in actual.facts.iter().zip([&a, &b_object, &a]) {
        assert!(std::ptr::eq(
            fact.downcast_ref::<String>().unwrap(),
            expected.as_ref()
        ));
    }
    assert_eq!(report.matches[&0].indicted_facts.len(), 2);
    assert_eq!(report.indictments.len(), 2);
}

//! Original real graph/explanation, zero-weight and atomic registration checks.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[test]
fn declarations_survive_disabled_scoring_and_explanations_retain_terminal_names() {
    let mut b = CatalogGraphBuilder::<SimpleScore>::new();
    let source = b.builder().source::<usize, i32>();
    let active = Rc::new(DeclaredConstraint::new("active", SimpleScore::new(10)).unwrap());
    let disabled = Rc::new(DeclaredConstraint::new("disabled", SimpleScore::new(0)).unwrap());
    let calls = Cell::new(0);
    let a = b
        .register_weighted(active.clone(), source.port(), |v| i64::from(*v))
        .unwrap();
    let d = b
        .register_weighted(disabled.clone(), source.port(), |_| {
            calls.set(calls.get() + 1);
            1
        })
        .unwrap();
    let mut g = b.build();
    let mut tx = g.graph_mut().transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.graph().score(), SimpleScore::new(30));
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(g.catalog().get("disabled").unwrap(), &disabled));
    assert_eq!(
        *g.catalog().get("disabled").unwrap().declared_weight(),
        SimpleScore::new(0)
    );
    let report = g
        .graph()
        .explain_constraint(&a.terminal, |v, _| (*v, vec![*v]))
        .unwrap();
    assert_eq!(report.name, "active");
    assert_eq!(report.score, SimpleScore::new(30));
    assert_eq!(report.matches.len(), 1);
    let report = g
        .graph()
        .explain_constraint(&d.terminal, |v, _| (*v, vec![*v]))
        .unwrap();
    assert_eq!(report.name, "disabled");
    assert!(report.matches.is_empty());
    assert_eq!(report.score, SimpleScore::new(0));
}
#[test]
fn invalid_duplicate_foreign_and_structural_registration_publish_no_orphans() {
    assert!(DeclaredConstraint::new("bad/name", SimpleScore::new(1)).is_err());
    let mut b = CatalogGraphBuilder::<SimpleScore>::new();
    let source = b.builder().source::<usize, i32>();
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let other = foreign.source::<usize, i32>();
    let declaration = Rc::new(DeclaredConstraint::new("valid", SimpleScore::new(1)).unwrap());
    assert!(
        b.register_weighted(declaration.clone(), other.port(), |_| 1)
            .is_err()
    );
    assert!(b.catalog().is_empty());
    let bad = Rc::new(
        DeclaredConstraint::new("structural", SimpleScore::new(1).with_structural(-1)).unwrap(),
    );
    assert!(matches!(
        b.register_weighted(bad, source.port(), |_| 1),
        Err(GraphError::StructuralConstraintWeight(_))
    ));
    assert!(b.catalog().is_empty());
    b.register_weighted(declaration.clone(), source.port(), |_| 1)
        .unwrap();
    let duplicate = Rc::new(DeclaredConstraint::new("valid", SimpleScore::new(9)).unwrap());
    assert!(matches!(
        b.register_weighted(duplicate, source.port(), |_| 9),
        Err(GraphError::DuplicateConstraint(_))
    ));
    assert_eq!(b.catalog().len(), 1);
    assert!(Rc::ptr_eq(b.catalog().get("valid").unwrap(), &declaration));
    let mut g = b.build();
    assert_eq!(g.graph().constraint_scores().count(), 1);
    let mut tx = g.graph_mut().transaction();
    tx.apply(&source, TupleChange::put(0, 2)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.graph().score(), SimpleScore::new(1));
    assert_eq!(g.graph().constraint_scores().count(), 1);
}

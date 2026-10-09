// Adapted from pinned Timefold Community SelectorBasedCompositeMoveTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn dummy(variable: usize) -> Move<&'static str> {
    Move::Change {
        variable,
        value: Some("dummy"),
    }
}
fn no_change() -> Move<&'static str> {
    Move::Composite(vec![])
}
#[test]
fn execute_each_child_once() {
    // Raw write journals replace mocked child execute invocations.
    let movement = compose_basic_moves([dummy(0), dummy(1), dummy(2)]);
    let mut assignment = Assignment::new(vec![None; 3]);
    let applied = assignment.apply(&movement).unwrap();
    for variable in 0..3 {
        assert_eq!(
            applied
                .prior_values()
                .iter()
                .filter(|w| w.variable == variable)
                .count(),
            1
        );
    }
    assert_eq!(applied.values(), &[Some("dummy"); 3]);
}
#[test]
fn build_empty() {
    assert_eq!(compose_basic_moves::<&str>(Vec::new()), no_change());
    assert_eq!(compose_basic_moves::<&str>([]), no_change());
}
#[test]
fn build_single() {
    assert!(matches!(
        compose_basic_moves(vec![dummy(0)]),
        Move::Change { variable: 0, .. }
    ));
    assert!(matches!(
        compose_basic_moves([dummy(0)]),
        Move::Change { variable: 0, .. }
    ));
}
#[test]
fn build_two() {
    for movement in [
        compose_basic_moves([dummy(0), no_change()]),
        compose_basic_moves([dummy(0), no_change()]),
    ] {
        let Move::Composite(children) = movement else {
            panic!("composite expected")
        };
        assert!(matches!(children[0], Move::Change { variable: 0, .. }));
        assert_eq!(children[1], no_change());
        assert_eq!(children.len(), 2);
    }
}
#[test]
fn equality_keeps_order_and_shape() {
    let movement = compose_basic_moves(vec![dummy(0), no_change()]);
    assert_eq!(movement, compose_basic_moves([dummy(0), no_change()]));
    let movement = compose_basic_moves([dummy(0), no_change()]);
    assert_ne!(movement, compose_basic_moves([no_change(), dummy(0)]));
    assert_ne!(movement, dummy(1));
}
#[test]
fn interconnected_children_read_preceding_writes() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2", "v3"]).with_initial("v1"),
        Variable::new(vec!["v1", "v2", "v3"]).with_initial("v2"),
    ])
    .unwrap();
    let mut a = p.initial_assignment();
    let mut draft = BasicMoveBuilder::new(&p, &a).unwrap();
    draft.change(0, Some("v3")).unwrap();
    draft.swap(0, 1).unwrap();
    assert_eq!(a.values(), &[Some("v1"), Some("v2")]);
    p.apply(&mut a, &draft.finish()).unwrap().commit();
    assert_eq!(a.values(), &[Some("v2"), Some("v3")]);
}

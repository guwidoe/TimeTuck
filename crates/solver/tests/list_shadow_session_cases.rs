// Adapted from SingleDirectionalParentVariableReferenceGraphTest and counting fixtures.
// Apache-2.0; see compat/provenance.json. Native typed previous-chain session replaces reflective graph selection.
use std::{cell::RefCell, collections::BTreeSet};
use timetuck::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Value(&'static str);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Owner(&'static str);
struct Solution;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<Owner>("entity"),
            PlanningVariableDefinition::shadow::<i32>("count"),
        ]),
    ])
    .unwrap()
}
fn verify(session: &ListShadowSession<'_, '_, Owner, Value, i32>, expected: &[Option<i32>]) {
    let values = session.values().unwrap();
    for (i, &count) in expected.iter().enumerate() {
        let key = [
            Value("v1"),
            Value("v2"),
            Value("v3"),
            Value("v4"),
            Value("v5"),
        ][i];
        assert_eq!(values[&key].value, count);
        assert!(!values[&key].inconsistent);
    }
}
#[test]
fn supplier_methods_are_only_called_once() {
    let s = schema();
    let list = s.entity::<Owner>().unwrap().variable("values").unwrap();
    let e = s.entity::<Value>().unwrap();
    let target = e.variable("count").unwrap();
    let previous = e.variable("previous").unwrap();
    let owner = e.variable("entity").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    let (v1, v2, v3, v4, v5) = (
        Value("v1"),
        Value("v2"),
        Value("v3"),
        Value("v4"),
        Value("v5"),
    );
    let (e1, e2) = (Owner("e1"), Owner("e2"));
    registry
        .reset_lists(list, [(e1, vec![v1]), (e2, vec![v2, v3, v4])])
        .unwrap();
    let called = RefCell::new(BTreeSet::new());
    let mut session = ListShadowSession::new(
        registry.list::<Owner, Value>(list).unwrap(),
        target,
        [v5, v4, v3, v2, v1],
        |c| {
            assert!(
                called.borrow_mut().insert(*c.value),
                "supplier called twice for {:?}",
                c.value
            );
            c.location
                .map(|_| c.previous_shadow.copied().unwrap_or(-1) + 1)
        },
    )
    .unwrap();
    verify(&session, &[Some(0), Some(0), Some(1), Some(2), None]);
    called.borrow_mut().clear();
    registry
        .update_lists(list, [(e1, vec![v1]), (e2, vec![v5, v3, v2, v4])])
        .unwrap();
    for (field, value) in [(previous, v2), (previous, v3), (owner, v5), (previous, v4)] {
        session.after_variable_changed(field, &value).unwrap();
    }
    assert!(!session.is_current());
    session.update_changed().unwrap();
    verify(&session, &[Some(0), Some(2), Some(1), Some(3), Some(0)]);
    assert!(!called.borrow().contains(&v1));
}
#[test]
fn elements_unassigned_in_same_pass_are_all_updated() {
    let s = schema();
    let list = s.entity::<Owner>().unwrap().variable("values").unwrap();
    let e = s.entity::<Value>().unwrap();
    let target = e.variable("count").unwrap();
    let previous = e.variable("previous").unwrap();
    let owner = e.variable("entity").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    let (v1, v2, v3, v4) = (Value("v1"), Value("v2"), Value("v3"), Value("v4"));
    let e1 = Owner("e1");
    registry
        .reset_lists(list, [(e1, vec![v1, v2, v3, v4])])
        .unwrap();
    let called = RefCell::new(BTreeSet::new());
    let mut session = ListShadowSession::new(
        registry.list::<Owner, Value>(list).unwrap(),
        target,
        [v1, v2, v3, v4],
        |c| {
            assert!(
                called.borrow_mut().insert(*c.value),
                "supplier called twice for {:?}",
                c.value
            );
            c.location
                .map(|_| c.previous_shadow.copied().unwrap_or(-1) + 1)
        },
    )
    .unwrap();
    verify(&session, &[Some(0), Some(1), Some(2), Some(3)]);
    called.borrow_mut().clear();
    registry.update_lists(list, [(e1, vec![v1, v4])]).unwrap();
    for value in [v2, v3] {
        session.after_variable_changed(owner, &value).unwrap();
        session.after_variable_changed(previous, &value).unwrap();
    }
    session.after_variable_changed(previous, &v4).unwrap();
    session.update_changed().unwrap();
    verify(&session, &[Some(0), None, None, Some(1)]);
    assert!(called.borrow().contains(&v2));
    assert!(called.borrow().contains(&v3));
}

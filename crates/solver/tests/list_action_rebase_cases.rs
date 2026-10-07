// Adapted from pinned Community ListVariableBeforeChangeActionTest.
// Apache-2.0; see compat/provenance.json.
use std::{collections::BTreeMap, sync::Arc};
use timetuck::*;
struct Solution;
struct Owner;
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Object {
    id: usize,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Owner>(vec![
        PlanningVariableDefinition::list::<Arc<Object>>("valueList", false),
    ])])
    .unwrap()
}
fn obj(id: usize) -> Arc<Object> {
    Arc::new(Object { id })
}
fn notifications<'s>(
    events: &[ListChangeNotification<'s, Arc<Object>>],
    field: PlanningVariableRef<'s>,
    owner: &Arc<Object>,
    mutated_to: usize,
    restored_to: usize,
) {
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[0],
        ListChangeNotification::Before {
            variable: field,
            entity: owner.clone(),
            from: 0,
            to: mutated_to
        }
    );
    assert_eq!(
        events[1],
        ListChangeNotification::After {
            variable: field,
            entity: owner.clone(),
            from: 0,
            to: restored_to
        }
    );
    assert_eq!(events[2], ListChangeNotification::UpdateShadows);
}
#[test]
fn undo_restores_and_notifies() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
    let r = ListChangeRecorder::new(f).unwrap();
    let e = obj(7);
    let old = obj(0);
    let new = obj(1);
    r.before(e.clone(), 0, 1, std::slice::from_ref(&old))
        .unwrap();
    r.after(e.clone(), 0, 1, std::slice::from_ref(&new))
        .unwrap();
    let mut lists = BTreeMap::from([(e.clone(), vec![new])]);
    let mut events = vec![];
    r.undo_changes(&mut lists, |event, _| events.push(event))
        .unwrap();
    assert_eq!(lists[&e], vec![old]);
    notifications(&events, f, &e, 1, 1);
}
#[test]
fn undo_insert_clears_inserted() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
    let r = ListChangeRecorder::new(f).unwrap();
    let e = obj(7);
    let inserted = obj(1);
    r.before(e.clone(), 0, 0, &[]).unwrap();
    r.after(e.clone(), 0, 1, std::slice::from_ref(&inserted))
        .unwrap();
    let mut lists = BTreeMap::from([(e.clone(), vec![inserted])]);
    let mut events = vec![];
    r.undo_changes(&mut lists, |event, _| events.push(event))
        .unwrap();
    assert!(lists[&e].is_empty());
    notifications(&events, f, &e, 1, 0);
}
#[test]
fn unclosed_action_fails() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
    let r = ListChangeRecorder::new(f).unwrap();
    let e = obj(7);
    let old = obj(0);
    r.before(e.clone(), 0, 1, &[old]).unwrap();
    let mut lists = BTreeMap::from([(e, vec![obj(1)])]);
    let error = r
        .undo_changes(&mut lists, |_, _| panic!("no callback"))
        .unwrap_err();
    assert_eq!(error, ListRecordingError::UnclosedBracket);
    assert!(
        error
            .to_string()
            .contains("was never closed by its after list change")
    );
}
#[test]
fn rebase_preserves_closed_action_and_working_identity() {
    let s = schema();
    let f = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
    let r = ListChangeRecorder::new(f).unwrap();
    let e = obj(7);
    let old = obj(0);
    let new = obj(1);
    r.before(e.clone(), 0, 1, std::slice::from_ref(&old))
        .unwrap();
    r.after(e.clone(), 0, 1, std::slice::from_ref(&new))
        .unwrap();
    let target_e = obj(7);
    let target_old = obj(0);
    let target_new = obj(1);
    let mut entities = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("id", |v: &Object| Some(v.id)),
    );
    entities.add(target_e.clone()).unwrap();
    let mut values = WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("id", |v: &Object| Some(v.id)),
    );
    values.add(target_old.clone()).unwrap();
    values.add(target_new.clone()).unwrap();
    let rebased = r
        .rebase(f, |e| entities.lookup(e), |v| values.lookup(v))
        .unwrap();
    let mut lists = BTreeMap::from([(target_e.clone(), vec![target_new])]);
    let mut events = vec![];
    rebased
        .undo_changes(&mut lists, |event, _| events.push(event))
        .unwrap();
    assert_eq!(lists[&target_e].len(), 1);
    assert!(Arc::ptr_eq(&lists[&target_e][0], &target_old));
    assert!(!Arc::ptr_eq(&lists[&target_e][0], &old));
    notifications(&events, f, &target_e, 1, 1);
    for event in &events[..2] {
        match event {
            ListChangeNotification::Before { entity, .. }
            | ListChangeNotification::After { entity, .. } => {
                assert!(Arc::ptr_eq(entity, &target_e));
                assert!(!Arc::ptr_eq(entity, &e));
            }
            _ => unreachable!(),
        }
    }
}

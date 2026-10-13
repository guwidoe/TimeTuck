// Original real-state recording/replay controls; no upstream method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
struct Solution;
fn planning_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<usize>(vec![
        PlanningVariableDefinition::basic::<usize>("first", true),
        PlanningVariableDefinition::basic::<usize>("second", false),
        PlanningVariableDefinition::shadow::<usize>("shadow"),
    ])])
    .unwrap()
}
#[test]
fn repeated_multi_field_changes_restore_intermediate_values_indexes_and_trackers() {
    let schema = planning_schema();
    let e = schema.entity::<usize>().unwrap();
    let first = e.variable("first").unwrap();
    let second = e.variable("second").unwrap();
    let mut r = VariableStateRegistry::new(&schema);
    r.reset_basic(first, [(0usize, Some(10usize))]).unwrap();
    r.reset_basic(second, [(0usize, Some(7usize))]).unwrap();
    let state = r.basic::<usize, usize>(first).unwrap();
    let tracker = r.tracker::<usize, usize>(first).unwrap();
    let mut record = BasicChangeRecorder::<usize, usize>::default();
    record.change(&mut r, first, 0, Some(20)).unwrap();
    record.change(&mut r, first, 0, Some(30)).unwrap();
    record.change(&mut r, first, 0, None).unwrap();
    record.change(&mut r, second, 0, Some(8)).unwrap();
    assert!(record.change(&mut r, first, 99, Some(1)).is_err());
    let mut undo = record.into_undo();
    assert_eq!(undo.remaining(), 4);
    let mut states = vec![];
    undo.execute(&mut r, |action, r| {
        states.push((
            action.variable.name().to_owned(),
            r.basic::<usize, usize>(first).unwrap().snapshot()[&0],
        ));
    })
    .unwrap();
    assert_eq!(
        states,
        [
            ("second".into(), None),
            ("first".into(), Some(30)),
            ("first".into(), Some(20)),
            ("first".into(), Some(10))
        ]
    );
    assert_eq!(state.inverse(&10), [0]);
    assert!(state.inverse(&20).is_empty());
    assert_eq!(
        r.basic::<usize, usize>(second).unwrap().snapshot()[&0],
        Some(7)
    );
    assert!(tracker.audit([(first, 0)]).is_empty());
    assert_eq!(undo.remaining(), 0);
    undo.execute(&mut r, |_, _| panic!("completed replay"))
        .unwrap();
}
#[test]
fn failed_action_keeps_only_remaining_prefix_for_retry() {
    let schema = planning_schema();
    let first = schema.entity::<usize>().unwrap().variable("first").unwrap();
    let mut r = VariableStateRegistry::new(&schema);
    r.reset_basic(first, [(0usize, Some(100usize))]).unwrap();
    let mut undo = RecordedBasicUndo::new(vec![
        BasicRestoreAction::new(first, 9usize, Some(9usize)).unwrap(),
        BasicRestoreAction::new(first, 0usize, Some(1usize)).unwrap(),
    ]);
    let mut observed = vec![];
    assert!(
        undo.execute(&mut r, |a, _| observed.push(a.entity))
            .is_err()
    );
    assert_eq!(observed, [0]);
    assert_eq!(undo.remaining(), 1);
    r.update_basic(first, [(0usize, Some(1usize)), (9, None)])
        .unwrap();
    undo.execute(&mut r, |a, _| observed.push(a.entity))
        .unwrap();
    assert_eq!(observed, [0, 9]);
    assert_eq!(undo.remaining(), 0);
    assert_eq!(
        r.basic::<usize, usize>(first).unwrap().snapshot()[&9],
        Some(9)
    );
}
#[test]
fn observer_panic_consumes_committed_action_and_invalid_metadata_never_records() {
    let schema = planning_schema();
    let e = schema.entity::<usize>().unwrap();
    let first = e.variable("first").unwrap();
    assert!(BasicRestoreAction::new(e.variable("shadow").unwrap(), 0usize, Some(1usize)).is_err());
    assert!(BasicRestoreAction::new(first, 0usize, Some("wrong value")).is_err());
    let foreign = planning_schema();
    let foreign_field = foreign
        .entity::<usize>()
        .unwrap()
        .variable("first")
        .unwrap();
    let mut r = VariableStateRegistry::new(&schema);
    r.reset_basic(first, [(0usize, Some(10usize))]).unwrap();
    let mut record = BasicChangeRecorder::<usize, usize>::default();
    assert!(
        record
            .change(&mut r, foreign_field, 0usize, Some(20usize))
            .is_err()
    );
    assert_eq!(record.into_undo().remaining(), 0);
    let mut undo = RecordedBasicUndo::new(vec![
        BasicRestoreAction::new(first, 0usize, Some(10usize)).unwrap(),
        BasicRestoreAction::new(first, 0usize, Some(20usize)).unwrap(),
    ]);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || undo.execute(&mut r, |_, _| panic!("observer"))
        ))
        .is_err()
    );
    assert_eq!(undo.remaining(), 1);
    assert_eq!(
        r.basic::<usize, usize>(first).unwrap().snapshot()[&0],
        Some(20)
    );
    let mut seen = vec![];
    undo.execute(&mut r, |a, _| seen.push(a.value)).unwrap();
    assert_eq!(seen, [Some(10)]);
}

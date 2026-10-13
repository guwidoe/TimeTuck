// Original native entity-batch refresh controls; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("value", true),
        PlanningVariableDefinition::shadow::<i64>("double"),
    ])])
    .unwrap()
}
#[test]
fn validates_whole_selection_and_refreshes_only_selected_owners_once() {
    let schema = schema();
    let fields = schema.entity::<Entity>().unwrap();
    let reads = [Cell::new(0), Cell::new(0)];
    let supplies = [Cell::new(0), Cell::new(0)];
    let mut builder = ShadowSessionBuilder::<usize, [i64; 2]>::new(&schema);
    let inputs: Vec<_> = (0..2)
        .map(|id| {
            let reads = &reads;
            builder
                .input(fields.variable("value").unwrap(), id, move |world| {
                    reads[id].set(reads[id].get() + 1);
                    Some(world[id])
                })
                .unwrap()
        })
        .collect();
    let outputs: Vec<_> = (0..2)
        .map(|id| {
            let input = inputs[id].clone();
            let supplies = &supplies;
            let output = builder
                .derive(fields.variable("double").unwrap(), id, move |_, values| {
                    supplies[id].set(supplies[id].get() + 1);
                    values.get(&input).unwrap().map(|v| v * 2)
                })
                .unwrap();
            builder
                .set_dependencies(&output, &[inputs[id].erase()])
                .unwrap();
            output
        })
        .collect();
    let mut session = builder.build();
    session.update(&[1, 2]).unwrap();
    for count in reads.iter().chain(&supplies) {
        count.set(0);
    }
    let mut other = ShadowSessionBuilder::<usize, [i64; 2]>::new(&schema);
    let foreign = other
        .input(fields.variable("value").unwrap(), 0, |_| Some(1i64))
        .unwrap();
    assert!(matches!(
        session.update_entities(&[3, 2], &[inputs[0].erase(), foreign.erase()]),
        Err(ShadowSessionError::ForeignNode)
    ));
    assert_eq!(session.read(&outputs[0]).unwrap().value, Some(&2));
    assert!(session.update_notified(&[1, 2]).unwrap().is_empty());
    assert_eq!(reads[0].get(), 0);
    assert_eq!(supplies[0].get(), 0);
    let changed = session
        .update_entities(
            &[3, 2],
            &[inputs[0].erase(), outputs[0].erase(), inputs[0].erase()],
        )
        .unwrap();
    assert!(!changed.is_empty());
    assert_eq!(reads[0].get(), 1);
    assert_eq!(supplies[0].get(), 1);
    assert_eq!(reads[1].get(), 0);
    assert_eq!(supplies[1].get(), 0);
    assert_eq!(session.read(&outputs[0]).unwrap().value, Some(&6));
    assert_eq!(session.read(&outputs[1]).unwrap().value, Some(&4));
}

#[test]
fn empty_selection_preserves_pending_work_and_failed_callbacks_retry() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("double")
        .unwrap();
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let output = builder
        .derive(field, 0, |world, _| {
            calls.set(calls.get() + 1);
            assert!(!fail.get(), "supplier failed");
            Some(*world)
        })
        .unwrap();
    let mut session = builder.build();
    session.update(&1).unwrap();
    session.mark_entities_dirty(&[output.erase()]).unwrap();
    let before = calls.get();
    assert_eq!(
        session
            .update_entities(&2, &[])
            .err()
            .expect("empty entity array must fail"),
        ShadowSessionError::EmptyEntityArray
    );
    assert_eq!(calls.get(), before);
    assert!(matches!(
        session.read(&output),
        Err(ShadowSessionError::NotCurrent)
    ));
    session.update_notified(&2).unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&2));
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || session.update_entities(&3, &[output.erase()])
        ))
        .is_err()
    );
    assert!(matches!(
        session.read(&output),
        Err(ShadowSessionError::NotCurrent)
    ));
    fail.set(false);
    session.update_notified(&3).unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&3));
}

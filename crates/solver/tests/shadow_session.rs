//! Original heterogeneous scheduling, native undo and failure recovery.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Route;
#[derive(Debug, Eq, PartialEq)]
struct Label(String); // Deliberately not Clone.
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Route>(vec![
        PlanningVariableDefinition::list::<u32>("visits", false),
        PlanningVariableDefinition::basic::<i64>("bias", false),
        PlanningVariableDefinition::shadow::<i64>("sum"),
        PlanningVariableDefinition::shadow::<i64>("double"),
        PlanningVariableDefinition::shadow::<Label>("label"),
        PlanningVariableDefinition::shadow::<bool>("flag"),
    ])])
    .unwrap()
}
struct World {
    assignment: ListAssignment<u32>,
    bias: i64,
}
#[test]
fn heterogeneous_chain_updates_once_stops_on_equal_results_and_restores_native_undo() {
    let s = schema();
    let e = s.entity::<Route>().unwrap();
    let p = ListProblem::new(vec![1u32, 2, 3], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let mut world = World {
        assignment: p.initial_assignment(),
        bias: 1,
    };
    let sums = &Cell::new(0);
    let doubles = &Cell::new(0);
    let labels = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&s);
    let list = b
        .list_input(e.variable("visits").unwrap(), 0, |w| {
            w.assignment.list(0).unwrap().to_vec()
        })
        .unwrap();
    let bias = b
        .input(e.variable("bias").unwrap(), 0, |w| Some(w.bias))
        .unwrap();
    let input = list.clone();
    let sum = b
        .derive(e.variable("sum").unwrap(), 0, move |_, i| {
            sums.set(sums.get() + 1);
            Some(
                i.get(&input)
                    .unwrap()
                    .unwrap()
                    .iter()
                    .map(|v| i64::from(*v))
                    .sum::<i64>(),
            )
        })
        .unwrap();
    let input = sum.clone();
    let double = b
        .derive(e.variable("double").unwrap(), 0, move |_, i| {
            doubles.set(doubles.get() + 1);
            i.get(&input).unwrap().map(|v| v * 2)
        })
        .unwrap();
    let left = double.clone();
    let right = bias.clone();
    let label = b
        .derive(e.variable("label").unwrap(), 0, move |_, i| {
            labels.set(labels.get() + 1);
            Some(Label(
                (i.get(&left).unwrap().unwrap() + i.get(&right).unwrap().unwrap()).to_string(),
            ))
        })
        .unwrap();
    b.set_dependencies(&sum, &[list.erase()]).unwrap();
    b.set_dependencies(&double, &[sum.erase()]).unwrap();
    b.set_dependencies(&label, &[double.erase(), bias.erase()])
        .unwrap();
    let mut session = b.build();
    assert!(matches!(
        session.read(&label),
        Err(ShadowSessionError::NotCurrent)
    ));
    assert_eq!(session.update(&world).unwrap().len(), 3);
    assert_eq!(session.read(&label).unwrap().value.unwrap().0, "7");
    assert_eq!((sums.get(), doubles.get(), labels.get()), (1, 1, 1));
    assert!(session.update(&world).unwrap().is_empty());
    assert_eq!((sums.get(), doubles.get(), labels.get()), (1, 1, 1));
    let saved = session.checkpoint().unwrap();
    let trial = p
        .apply(
            &mut world.assignment,
            &ListMove::Assign {
                value: 3,
                destination: ListPosition {
                    entity: 0,
                    index: 2,
                },
            },
        )
        .unwrap();
    let candidate = World {
        assignment: trial.clone(),
        bias: 1,
    };
    assert_eq!(session.update(&candidate).unwrap().len(), 3);
    assert_eq!(session.read(&label).unwrap().value.unwrap().0, "13");
    assert!(matches!(
        session.restore(&saved, &candidate),
        Err(ShadowSessionError::SnapshotMismatch)
    ));
    drop(trial);
    let calls = (sums.get(), doubles.get(), labels.get());
    session.restore(&saved, &world).unwrap();
    assert_eq!((sums.get(), doubles.get(), labels.get()), calls);
    assert_eq!(session.read(&label).unwrap().value.unwrap().0, "7");
    p.apply(
        &mut world.assignment,
        &ListMove::Swap {
            left: ListPosition {
                entity: 0,
                index: 0,
            },
            right: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    )
    .unwrap()
    .commit();
    assert!(session.update(&world).unwrap().is_empty());
    assert_eq!(
        (sums.get(), doubles.get(), labels.get()),
        (calls.0 + 1, calls.1, calls.2)
    );
    session.set_dependencies(&sum, &[double.erase()]).unwrap();
    session.update(&world).unwrap();
    assert!(session.read(&sum).unwrap().inconsistent);
    assert!(session.read(&double).unwrap().inconsistent);
    assert!(session.read(&label).unwrap().inconsistent);
    assert!(session.read(&label).unwrap().value.is_none());
    assert_eq!(
        (sums.get(), doubles.get(), labels.get()),
        (calls.0 + 1, calls.1, calls.2)
    );
    session.set_dependencies(&sum, &[list.erase()]).unwrap();
    session.update(&world).unwrap();
    assert!(!session.read(&label).unwrap().inconsistent);
    assert_eq!(session.read(&label).unwrap().value.unwrap().0, "7");
    assert_eq!(
        session.variable(&label.erase()).unwrap(),
        e.variable("label").unwrap()
    );
    assert_eq!(*session.entity(&label.erase()).unwrap(), 0);
}
#[test]
fn supplier_panics_preserve_cache_and_pending_work_then_retry() {
    let s = schema();
    let e = s.entity::<Route>().unwrap();
    let fail = &Cell::new(false);
    let calls = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, i64>::new(&s);
    let source = b
        .input(e.variable("bias").unwrap(), 0, |w| Some(*w))
        .unwrap();
    let input = source.clone();
    let target = b
        .derive(e.variable("sum").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            assert!(!fail.get(), "supplier failure");
            i.get(&input).unwrap().copied()
        })
        .unwrap();
    b.set_dependencies(&target, &[source.erase()]).unwrap();
    let mut session = b.build();
    session.update(&1).unwrap();
    let saved = session.checkpoint().unwrap();
    fail.set(true);
    session.mark_dirty(&target).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| session.update(&2))).is_err());
    assert!(matches!(
        session.read(&target),
        Err(ShadowSessionError::NotCurrent)
    ));
    fail.set(false);
    session.update(&2).unwrap();
    assert_eq!(session.read(&target).unwrap().value, Some(&2));
    let count = calls.get();
    session.restore(&saved, &1).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(session.read(&target).unwrap().value, Some(&1));
}
#[test]
fn metadata_handles_and_declared_input_access_are_checked() {
    let s = schema();
    let e = s.entity::<Route>().unwrap();
    let other = schema();
    let mut b = ShadowSessionBuilder::<usize, i64>::new(&s);
    assert!(
        b.input(e.variable("bias").unwrap(), 0, |_| Some(0u32))
            .is_err()
    );
    assert!(
        b.derive(e.variable("bias").unwrap(), 0, |_, _| Some(0i64))
            .is_err()
    );
    assert!(
        b.list_input(e.variable("bias").unwrap(), 0, |_| vec![0i64])
            .is_err()
    );
    assert!(
        b.input(
            other.entity::<Route>().unwrap().variable("bias").unwrap(),
            0,
            |_| Some(0i64)
        )
        .is_err()
    );
    let source = b
        .input(e.variable("bias").unwrap(), 0, |w| Some(*w))
        .unwrap();
    assert!(
        b.input(e.variable("bias").unwrap(), 0, |_| Some(0i64))
            .is_err()
    );
    let hidden = source.clone();
    let source_field = e.variable("bias").unwrap();
    let foreign_field = other.entity::<Route>().unwrap().variable("bias").unwrap();
    let target = b
        .derive(e.variable("flag").unwrap(), 0, move |_, i| {
            assert!(matches!(
                i.for_field::<i64>(foreign_field),
                Err(ShadowSessionError::Metadata(_))
            ));
            assert!(matches!(
                i.for_field::<bool>(source_field),
                Err(ShadowSessionError::WrongType)
            ));
            Some(matches!(
                i.get(&hidden),
                Err(ShadowSessionError::NotDependency)
            ))
        })
        .unwrap();
    assert!(b.set_dependencies(&source, &[target.erase()]).is_err());
    let mut other_builder = ShadowSessionBuilder::<usize, i64>::new(&s);
    let foreign = other_builder
        .input(e.variable("bias").unwrap(), 0, |w| Some(*w))
        .unwrap();
    assert!(b.set_dependencies(&target, &[foreign.erase()]).is_err());
    let mut session = b.build();
    session.update(&4).unwrap();
    assert_eq!(session.read(&target).unwrap().value, Some(&true));
    assert!(session.read(&foreign).is_err());
    let saved = session.checkpoint().unwrap();
    let mut foreign_session = other_builder.build();
    foreign_session.update(&4).unwrap();
    assert!(foreign_session.restore(&saved, &4).is_err());
}
#[test]
fn absent_inputs_remain_consistent_and_reader_failure_does_not_publish_a_prefix() {
    let s = schema();
    let e = s.entity::<Route>().unwrap();
    let fail = &Cell::new(false);
    let calls = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, Option<i64>>::new(&s);
    let source = b
        .input(e.variable("bias").unwrap(), 0, move |w| {
            assert!(!fail.get(), "reader failure");
            *w
        })
        .unwrap();
    let input = source.clone();
    let target = b
        .derive(e.variable("sum").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            i.get(&input).unwrap().map(|v| v * 2)
        })
        .unwrap();
    b.set_dependencies(&target, &[source.erase()]).unwrap();
    let mut session = b.build();
    session.update(&None).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(session.read(&target).unwrap().value.is_none());
    assert!(!session.read(&target).unwrap().inconsistent);
    session.update(&Some(7)).unwrap();
    assert_eq!(session.read(&target).unwrap().value, Some(&14));
    fail.set(true);
    let before = calls.get();
    assert!(catch_unwind(AssertUnwindSafe(|| session.update(&Some(9)))).is_err());
    assert_eq!(calls.get(), before);
    assert_eq!(session.read(&target).unwrap().value, Some(&14));
    fail.set(false);
    session.update(&None).unwrap();
    assert!(session.read(&target).unwrap().value.is_none());
    assert!(!session.read(&target).unwrap().inconsistent);
}

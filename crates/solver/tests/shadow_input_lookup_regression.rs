//! Original typed-handle lookup cases across sparse, reordered and restored inputs.
use std::{
    cell::Cell,
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Entity;
#[derive(Clone)]
struct World {
    values: Vec<Option<i64>>,
    dependencies: Vec<usize>,
    cycle: bool,
    fail: bool,
}
#[test]
fn typed_reads_match_registration_order_through_rebinding_cycles_and_restore() {
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
        PlanningVariableDefinition::basic::<i64>("other", false),
        PlanningVariableDefinition::shadow::<i64>("sum"),
    ])])
    .unwrap();
    let entity = schema.entity::<Entity>().unwrap();
    let field = entity.variable("value").unwrap();
    let mut foreign = ShadowSessionBuilder::<usize, World>::new(&schema);
    let foreign_node = foreign.input(field, 99999, |_| Some(0i64)).unwrap();
    let reads = Cell::new(0);
    let suppliers = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, World>::new(&schema);
    let mut handles = Vec::new();
    let mut hidden = Vec::new();
    const N: usize = 257;
    for i in 0..N {
        let reads = &reads;
        // Reverse entity keys and interleaved unrelated fields distinguish
        // registration IDs from domain-key order and dense dependency positions.
        handles.push(
            builder
                .input(field, N - i, move |w| {
                    reads.set(reads.get() + 1);
                    w.values[i]
                })
                .unwrap(),
        );
        if i % 7 == 0 {
            hidden.push(
                builder
                    .input(entity.variable("other").unwrap(), N - i, |_| Some(-1i64))
                    .unwrap(),
            );
        }
    }
    let supplier_handles = handles.clone();
    let supplier_calls = &suppliers;
    let sum = builder
        .derive(entity.variable("sum").unwrap(), 0, move |world, inputs| {
            supplier_calls.set(supplier_calls.get() + 1);
            assert_eq!(
                inputs.get(&foreign_node),
                Err(ShadowSessionError::ForeignNode)
            );
            for node in &hidden {
                assert_eq!(inputs.get(node), Err(ShadowSessionError::NotDependency));
            }
            let selected: BTreeSet<_> = world.dependencies.iter().copied().collect();
            let expected: Vec<_> = selected.iter().map(|&i| world.values[i]).collect();
            assert_eq!(
                inputs
                    .for_field::<i64>(field)
                    .unwrap()
                    .into_iter()
                    .map(|v| v.copied())
                    .collect::<Vec<_>>(),
                expected
            );
            let mut total = 0;
            for (i, node) in supplier_handles.iter().enumerate().rev() {
                if selected.contains(&i) {
                    assert_eq!(inputs.get(node).unwrap().copied(), world.values[i]);
                    total += inputs.get(node).unwrap().copied().unwrap_or(0);
                } else {
                    assert_eq!(inputs.get(node), Err(ShadowSessionError::NotDependency));
                }
            }
            assert!(!world.fail, "supplier panic after typed reads");
            Some(total)
        })
        .unwrap();
    let binding_handles = handles.clone();
    let self_ref = sum.erase();
    builder
        .bind_dependencies(&sum, move |world| {
            let mut result: Vec<_> = world
                .dependencies
                .iter()
                .map(|&i| binding_handles[i].erase())
                .collect();
            if world.cycle {
                result.push(self_ref.clone());
            }
            Ok(result)
        })
        .unwrap();
    let mut session = builder.build();
    let mut world = World {
        values: (0..N)
            .map(|i| if i % 11 == 0 { None } else { Some(i as i64) })
            .collect(),
        dependencies: vec![],
        cycle: false,
        fail: false,
    };
    session.update(&world).unwrap();
    assert_eq!(session.read(&sum).unwrap().value, Some(&0));
    for dependencies in [
        vec![N - 1],
        vec![0, N / 2, N - 1, 0],
        (0..N).rev().chain([N - 1, 0]).collect(),
    ] {
        world.dependencies = dependencies;
        let before = suppliers.get();
        session.mark_dirty(&sum).unwrap();
        session.update_notified(&world).unwrap();
        assert_eq!(suppliers.get(), before + 1);
        let expected: i64 = world
            .dependencies
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|i| world.values[i])
            .sum();
        assert_eq!(session.read(&sum).unwrap().value, Some(&expected));
    }
    // Full polling and one notified reader both feed the same typed lookup path.
    world.values[17] = Some(1000);
    let before = reads.get();
    session.update(&world).unwrap();
    assert_eq!(reads.get(), before + N);
    world.values[19] = Some(2000);
    let before = reads.get();
    session.mark_dirty(&handles[19]).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(reads.get(), before + 1);
    let saved_world = world.clone();
    let saved = session.checkpoint().unwrap();
    let saved_value = *session.read(&sum).unwrap().value.unwrap();
    world.cycle = true;
    let before = suppliers.get();
    session.mark_dirty(&sum).unwrap();
    session.update_notified(&world).unwrap();
    assert!(session.read(&sum).unwrap().inconsistent);
    assert_eq!(suppliers.get(), before);
    world.cycle = false;
    session.mark_dirty(&sum).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(suppliers.get(), before + 1);
    world.dependencies.reverse();
    session.mark_dirty(&sum).unwrap();
    session.update_notified(&world).unwrap();
    world.fail = true;
    session.mark_dirty(&sum).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| session.update_notified(&world))).is_err());
    let before = suppliers.get();
    session.restore(&saved, &saved_world).unwrap();
    assert_eq!(suppliers.get(), before);
    assert_eq!(session.read(&sum).unwrap().value, Some(&saved_value));
    session.mark_dirty(&sum).unwrap();
    session.update_notified(&saved_world).unwrap();
    assert_eq!(session.read(&sum).unwrap().value, Some(&saved_value));
}

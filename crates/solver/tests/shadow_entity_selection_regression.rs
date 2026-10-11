//! Original compiled entity selection and stream lifecycle regressions.
use ShadowEntitySelection::{
    AssignedAndConsistent as Assigned, ConsistentIncludingUnassigned as Consistent,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Task;
struct Other;
fn make_schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Task>(vec![
            PlanningVariableDefinition::basic::<i64>("left", true),
            PlanningVariableDefinition::basic::<i64>("right", false),
            PlanningVariableDefinition::shadow::<i64>("end"),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::basic::<i64>(
            "left", false,
        )]),
    ])
    .unwrap()
}
#[derive(Clone)]
struct World {
    left: Vec<Option<i64>>,
    right: Vec<Option<i64>>,
    cycle: bool,
}
#[test]
fn all_basic_fields_and_only_selected_type_control_eligibility_without_callbacks() {
    let schema = make_schema();
    let task = schema.entity::<Task>().unwrap();
    let calls = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let left = b
        .input(task.variable("left").unwrap(), 7, |w| w.left[0])
        .unwrap();
    b.input(task.variable("right").unwrap(), 7, |w| w.right[0])
        .unwrap();
    let derived = b
        .derive(task.variable("end").unwrap(), 7, |_, _| {
            calls.set(calls.get() + 1);
            None::<i64>
        })
        .unwrap();
    let other = b
        .input(
            schema.entity::<Other>().unwrap().variable("left").unwrap(),
            7,
            |_| None::<i64>,
        )
        .unwrap();
    let filter = b.entity_filter(task, &[]).unwrap();
    assert_eq!(filter.entity_count(), 1);
    let other_filter = b
        .entity_filter(schema.entity::<Other>().unwrap(), &[])
        .unwrap();
    let mut session = b.build();
    assert!(matches!(
        session.selected_entities(&filter, Assigned),
        Err(ShadowSessionError::NotCurrent)
    ));
    let mut world = World {
        left: vec![Some(1)],
        right: vec![None],
        cycle: false,
    };
    session.update(&world).unwrap();
    assert!(
        session
            .selected_entities(&filter, Assigned)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        session.selected_entities(&filter, Consistent).unwrap(),
        [&7]
    );
    world.right[0] = Some(2);
    session.update(&world).unwrap();
    let count = calls.get();
    for _ in 0..100 {
        assert!(
            session
                .entity_is_selected(&filter, &left.erase(), Assigned)
                .unwrap()
        );
        assert_eq!(session.selected_entities(&filter, Assigned).unwrap(), [&7]);
        assert!(
            session
                .selected_entities(&other_filter, Assigned)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(calls.get(), count);
    assert!(
        session
            .entity_is_selected(&filter, &other.erase(), Assigned)
            .is_err()
    );
    // A consistent absent derived result is not an assignment failure.
    assert!(session.read(&derived).unwrap().value.is_none());
    let saved = session.checkpoint().unwrap();
    session.mark_dirty(&left).unwrap();
    assert!(matches!(
        session.selected_entities(&filter, Assigned),
        Err(ShadowSessionError::NotCurrent)
    ));
    world.left[0] = None;
    session.update_notified(&world).unwrap();
    assert!(
        session
            .selected_entities(&filter, Assigned)
            .unwrap()
            .is_empty()
    );
    world.left[0] = Some(1);
    session.restore(&saved, &world).unwrap();
    assert_eq!(session.selected_entities(&filter, Assigned).unwrap(), [&7]);
}
#[test]
fn incomplete_foreign_and_stale_registrations_fail_before_stream_callbacks() {
    let schema = make_schema();
    let task = schema.entity::<Task>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&schema);
    b.input(task.variable("left").unwrap(), 0, |_| Some(1i64))
        .unwrap();
    assert!(b.entity_filter(task, &[]).is_err());
    b.input(task.variable("right").unwrap(), 0, |_| Some(2i64))
        .unwrap();
    let filter = b.entity_filter(task, &[]).unwrap();
    b.derive(task.variable("end").unwrap(), 0, |_, _| Some(3i64))
        .unwrap();
    let mut graph = StreamGraphBuilder::<SimpleScore>::new();
    let source = graph.source::<usize, ()>();
    assert!(matches!(
        graph.map_shadow_entities(source.port(), b, filter, Assigned, |_, _, _| Ok(1)),
        Err(GraphError::Dependency(_))
    ));
    let mut a = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let a_left = a
        .input(task.variable("left").unwrap(), 0, |_| Some(1i64))
        .unwrap();
    a.input(task.variable("right").unwrap(), 0, |_| Some(2i64))
        .unwrap();
    let filter = a.entity_filter(task, &[]).unwrap();
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&schema);
    b.input(task.variable("left").unwrap(), 0, |_| Some(1i64))
        .unwrap();
    b.input(task.variable("right").unwrap(), 0, |_| Some(2i64))
        .unwrap();
    let mut session = b.build();
    session.update(&()).unwrap();
    assert_eq!(
        session.selected_entities(&filter, Assigned),
        Err(ShadowSessionError::ForeignNode)
    );
    assert_eq!(
        session.entity_is_selected(&filter, &a_left.erase(), Assigned),
        Err(ShadowSessionError::ForeignNode)
    );
    let foreign_schema = make_schema(); // Canonical schema identity, not type-name equality.
    assert!(
        a.entity_filter(foreign_schema.entity::<Task>().unwrap(), &[])
            .is_err()
    );
}
#[test]
fn entity_streams_keep_stable_ids_independent_worlds_and_atomic_undo() {
    let schema = make_schema();
    let task = schema.entity::<Task>().unwrap();
    let projection_calls = Cell::new(0);
    let fail = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let mut ends = vec![];
    for key in 0..3 {
        let left = b
            .input(task.variable("left").unwrap(), key, move |w| w.left[key])
            .unwrap();
        let right = b
            .input(task.variable("right").unwrap(), key, move |w| w.right[key])
            .unwrap();
        let l = left.clone();
        let r = right.clone();
        let end = b
            .derive(task.variable("end").unwrap(), key, move |_, i| {
                Some(
                    i.get(&l).unwrap().copied().unwrap_or(0)
                        + i.get(&r).unwrap().copied().unwrap_or(0),
                )
            })
            .unwrap();
        let me = end.erase();
        let inputs = vec![left.erase(), right.erase()];
        b.bind_dependencies(&end, move |w| {
            Ok(if key == 1 && w.cycle {
                vec![me.clone()]
            } else {
                inputs.clone()
            })
        })
        .unwrap();
        ends.push(end);
    }
    let filter = b.entity_filter(task, &[]).unwrap();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, World>();
    let selected = builder
        .map_shadow_entities(source.port(), b, filter, Assigned, |_, key, session| {
            projection_calls.set(projection_calls.get() + 1);
            if fail.get() == 1 {
                return Err(ShadowSessionError::WrongType);
            }
            assert!(fail.get() != 2, "projection panic");
            Ok(*session.read(&ends[*key])?.value.unwrap())
        })
        .unwrap();
    let dataset = builder.dataset(&selected).unwrap();
    builder
        .score("end", &selected, |v| SimpleScore::new(*v))
        .unwrap();
    let mut graph = builder.build();
    let mut world = World {
        left: vec![Some(1), Some(2), Some(3)],
        right: vec![Some(10), Some(20), Some(30)],
        cycle: false,
    };
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(4, world.clone()))
        .unwrap();
    tx.apply(&source, TupleChange::put(9, world.clone()))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(132));
    let before = graph.dataset_rows(&dataset).unwrap().clone();
    {
        let mut tx = graph.transaction();
        world.left[0] = None;
        world.cycle = true;
        let count = projection_calls.get();
        tx.apply(&source, TupleChange::put(4, world.clone()))
            .unwrap();
        assert_eq!(projection_calls.get() - count, 1);
        let data = tx.graph().unwrap().dataset_rows(&dataset).unwrap();
        assert_eq!(
            data.keys().copied().collect::<Vec<_>>(),
            [(4, 2), (9, 0), (9, 1), (9, 2)]
        );
        assert_eq!(data[&(4, 2)], 33);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(99));
    }
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
    world.left[0] = Some(1);
    world.cycle = false;
    for mode in [1, 2] {
        fail.set(mode);
        let mut tx = graph.transaction();
        if mode == 1 {
            assert!(
                tx.apply(&source, TupleChange::put(4, world.clone()))
                    .is_err()
            );
        } else {
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || tx.apply(&source, TupleChange::put(4, world.clone()))
                ))
                .is_err()
            );
        }
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
    }
    fail.set(0);
    let mut tx = graph.transaction();
    world.cycle = true;
    tx.apply(&source, TupleChange::put(4, world.clone()))
        .unwrap();
    tx.commit().unwrap();
    assert!(!graph.dataset_rows(&dataset).unwrap().contains_key(&(4, 1)));
    let mut tx = graph.transaction();
    world.cycle = false;
    tx.apply(&source, TupleChange::put(4, world.clone()))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
    let count = projection_calls.get();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(4)).unwrap();
    tx.commit().unwrap();
    assert_eq!(projection_calls.get(), count);
    assert_eq!(graph.score(), SimpleScore::new(66));
    graph.assert_consistent().unwrap();
}

#[test]
fn cycle_descendants_and_manual_recovery_use_the_same_current_cache() {
    struct CycleModel;
    struct Item;
    let schema =
        PlanningSchema::new::<CycleModel>(vec![PlanningEntityDefinition::of::<Item>(vec![
            PlanningVariableDefinition::basic::<i64>("value", false),
            PlanningVariableDefinition::shadow::<i64>("end"),
        ])])
        .unwrap();
    let fields = schema.entity::<Item>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let mut outputs = vec![];
    for key in 0..3 {
        b.input(fields.variable("value").unwrap(), key, |_| Some(1i64))
            .unwrap();
        outputs.push(
            b.derive(fields.variable("end").unwrap(), key, |_, _| Some(2i64))
                .unwrap(),
        );
    }
    b.set_dependencies(&outputs[1], &[outputs[0].erase()])
        .unwrap();
    b.set_dependencies(&outputs[2], &[outputs[1].erase()])
        .unwrap();
    let filter = b.entity_filter(fields, &[]).unwrap();
    let mut session = b.build();
    session.update(&()).unwrap();
    assert_eq!(
        session.selected_entities(&filter, Assigned).unwrap(),
        [&0, &1, &2]
    );
    let saved = session.checkpoint().unwrap();
    session
        .set_dependencies(&outputs[0], &[outputs[1].erase()])
        .unwrap();
    assert!(matches!(
        session.selected_entities(&filter, Consistent),
        Err(ShadowSessionError::NotCurrent)
    ));
    session.update_notified(&()).unwrap();
    assert!(
        session
            .selected_entities(&filter, Consistent)
            .unwrap()
            .is_empty()
    ); // Includes acyclic descendant 2.
    session.restore(&saved, &()).unwrap();
    assert_eq!(
        session.selected_entities(&filter, Assigned).unwrap(),
        [&0, &1, &2]
    );
}

#[test]
fn list_owner_bindings_are_explicit_unique_and_optional_for_consistency_only() {
    struct ListModel;
    struct Route;
    let schema = PlanningSchema::new::<ListModel>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<u32>(
            "list", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![
            PlanningVariableDefinition::shadow::<usize>("owner"),
            PlanningVariableDefinition::shadow::<usize>("index"),
        ]),
    ])
    .unwrap();
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let route = schema.entity::<Route>().unwrap();
    let value = schema.entity::<u32>().unwrap();
    let list = b
        .list_input(route.variable("list").unwrap(), 0, |_| Vec::<u32>::new())
        .unwrap();
    let owner = b
        .input(value.variable("owner").unwrap(), 0, |_| None::<usize>)
        .unwrap();
    let index = b
        .input(value.variable("index").unwrap(), 0, |_| None::<usize>)
        .unwrap();
    assert!(
        b.entity_filter(value, &[owner.erase(), owner.erase()])
            .is_err()
    );
    assert!(
        b.entity_filter(value, &[owner.erase(), index.erase()])
            .is_err()
    );
    assert!(b.entity_filter(value, &[list.erase()]).is_err());
    assert!(b.entity_filter(route, &[owner.erase()]).is_err());
    let values = b.entity_filter(value, &[]).unwrap();
    let routes = b.entity_filter(route, &[]).unwrap();
    let mut session = b.build();
    session.update(&()).unwrap();
    assert_eq!(session.selected_entities(&routes, Assigned).unwrap(), [&0]); // Empty list owner is assigned.
    assert_eq!(
        session.selected_entities(&values, Consistent).unwrap(),
        [&0]
    );
    assert!(session.selected_entities(&values, Assigned).is_err());
}

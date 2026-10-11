// Adapted from Timefold Community EntityForEachFilterTest at
// 8b815bc6b5f3eb81feedc94dea0885a93b267762, Apache-2.0.
// See THIRD_PARTY_NOTICES.md and compat/provenance.json.
use ShadowEntitySelection::{
    AssignedAndConsistent as Assigned, ConsistentIncludingUnassigned as Consistent,
};
use timetuck::*;
struct Model;
struct Entity;
struct Owner;
#[derive(Clone, Eq, PartialEq)]
struct Value;
#[derive(Clone, Default)]
struct World {
    value: Option<i64>,
    owner: Option<usize>,
    cycle: bool,
}
fn basic_schema(declarative: bool) -> PlanningSchema {
    let mut fields = vec![PlanningVariableDefinition::basic::<i64>("value", false)];
    if declarative {
        fields.push(PlanningVariableDefinition::shadow::<i64>("durationInDays"));
    }
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(fields)]).unwrap()
}
fn list_schema(declarative: bool, inverse: bool) -> PlanningSchema {
    let mut fields = vec![PlanningVariableDefinition::shadow::<usize>("index")];
    if inverse {
        fields.push(PlanningVariableDefinition::shadow::<usize>("entity"));
    }
    if declarative {
        fields.push(PlanningVariableDefinition::shadow::<i64>(
            "serviceFinishTime",
        ));
    }
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "valueList",
            true,
        )]),
        PlanningEntityDefinition::of::<Value>(fields),
    ])
    .unwrap()
}
#[test]
fn filters_for_basic_entities() {
    let schema = basic_schema(false);
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, World>::new(&schema);
    let value = builder
        .input(entity.variable("value").unwrap(), "entity", |w| w.value)
        .unwrap();
    let filter = builder.entity_filter(entity, &[]).unwrap();
    assert!(!filter.requires_consistency()); // No consistency predicate in source.
    let mut session = builder.build();
    let mut world = World::default();
    session.update(&world).unwrap();
    assert!(
        !session
            .entity_is_selected(&filter, &value.erase(), Assigned)
            .unwrap()
    );
    world.value = Some(1);
    session.update(&world).unwrap();
    assert!(
        session
            .entity_is_selected(&filter, &value.erase(), Assigned)
            .unwrap()
    );
}
#[test]
fn filters_for_list_value_entities() {
    let schema = list_schema(false, true);
    let entity = schema.entity::<Value>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, World>::new(&schema);
    let list = builder
        .list_input(
            schema
                .entity::<Owner>()
                .unwrap()
                .variable("valueList")
                .unwrap(),
            "entity",
            |w| w.owner.map(|_| Value).into_iter().collect(),
        )
        .unwrap();
    let owner = builder
        .input(entity.variable("entity").unwrap(), "value", |w| w.owner)
        .unwrap();
    let filter = builder.entity_filter(entity, &[owner.erase()]).unwrap();
    assert!(!filter.requires_consistency());
    let mut session = builder.build();
    let mut world = World::default();
    session.update(&world).unwrap();
    assert_eq!(session.read(&list).unwrap().value.unwrap().len(), 0);
    assert!(
        !session
            .entity_is_selected(&filter, &owner.erase(), Assigned)
            .unwrap()
    );
    world.owner = Some(0);
    session.update(&world).unwrap();
    assert_eq!(session.read(&list).unwrap().value.unwrap().len(), 1);
    assert!(
        session
            .entity_is_selected(&filter, &owner.erase(), Assigned)
            .unwrap()
    );
}
#[test]
fn filters_for_declarative_basic_entities() {
    let schema = basic_schema(true);
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, World>::new(&schema);
    let value = builder
        .input(entity.variable("value").unwrap(), "entity", |w| w.value)
        .unwrap();
    let input = value.clone();
    let duration = builder
        .derive(
            entity.variable("durationInDays").unwrap(),
            "entity",
            move |_, i| Some(i.get(&input).unwrap().copied().unwrap_or(0) / 86400),
        )
        .unwrap();
    let loop_node = duration.erase();
    let source = value.erase();
    builder
        .bind_dependencies(&duration, move |w| {
            Ok(vec![if w.cycle {
                loop_node.clone()
            } else {
                source.clone()
            }])
        })
        .unwrap();
    let filter = builder.entity_filter(entity, &[]).unwrap();
    assert!(filter.requires_consistency());
    let mut session = builder.build();
    let mut world = World::default();
    session.update(&world).unwrap();
    assert!(
        !session
            .entity_is_selected(&filter, &value.erase(), Assigned)
            .unwrap()
    );
    assert!(
        session
            .entity_is_selected(&filter, &value.erase(), Consistent)
            .unwrap()
    );
    world.value = Some(3600);
    session.update(&world).unwrap();
    assert!(
        session
            .entity_is_selected(&filter, &value.erase(), Assigned)
            .unwrap()
    );
    assert!(
        session
            .entity_is_selected(&filter, &value.erase(), Consistent)
            .unwrap()
    );
    world.cycle = true;
    session.update(&world).unwrap();
    assert!(
        !session
            .entity_is_selected(&filter, &value.erase(), Assigned)
            .unwrap()
    );
    assert!(
        !session
            .entity_is_selected(&filter, &value.erase(), Consistent)
            .unwrap()
    );
}
#[test]
fn filters_for_declarative_list_value_entities() {
    let schema = list_schema(true, true);
    let entity = schema.entity::<Value>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, World>::new(&schema);
    let list = builder
        .list_input(
            schema
                .entity::<Owner>()
                .unwrap()
                .variable("valueList")
                .unwrap(),
            "entity",
            |w| w.owner.map(|_| Value).into_iter().collect(),
        )
        .unwrap();
    let owner = builder
        .input(entity.variable("entity").unwrap(), "value", |w| w.owner)
        .unwrap();
    let finish = builder
        .derive(
            entity.variable("serviceFinishTime").unwrap(),
            "value",
            |_, _| Some(0i64),
        )
        .unwrap();
    let loop_node = finish.erase();
    let source = owner.erase();
    builder
        .bind_dependencies(&finish, move |w| {
            Ok(vec![if w.cycle {
                loop_node.clone()
            } else {
                source.clone()
            }])
        })
        .unwrap();
    let filter = builder.entity_filter(entity, &[owner.erase()]).unwrap();
    assert!(filter.requires_consistency());
    let mut session = builder.build();
    let mut world = World::default();
    session.update(&world).unwrap();
    assert!(session.read(&list).unwrap().value.unwrap().is_empty());
    assert!(
        !session
            .entity_is_selected(&filter, &owner.erase(), Assigned)
            .unwrap()
    );
    assert!(
        session
            .entity_is_selected(&filter, &owner.erase(), Consistent)
            .unwrap()
    );
    world.owner = Some(0);
    session.update(&world).unwrap();
    assert_eq!(session.read(&list).unwrap().value.unwrap().len(), 1);
    assert!(
        session
            .entity_is_selected(&filter, &owner.erase(), Assigned)
            .unwrap()
    );
    assert!(
        session
            .entity_is_selected(&filter, &owner.erase(), Consistent)
            .unwrap()
    );
    world.cycle = true;
    session.update(&world).unwrap();
    assert!(
        !session
            .entity_is_selected(&filter, &owner.erase(), Assigned)
            .unwrap()
    );
    assert!(
        !session
            .entity_is_selected(&filter, &owner.erase(), Consistent)
            .unwrap()
    );
}
#[test]
fn filters_for_list_value_entities_without_inverse() {
    let schema = list_schema(false, false);
    let entity = schema.entity::<Value>().unwrap();
    let mut builder = ShadowSessionBuilder::<&str, World>::new(&schema);
    builder
        .list_input(
            schema
                .entity::<Owner>()
                .unwrap()
                .variable("valueList")
                .unwrap(),
            "entity",
            |_| Vec::<Value>::new(),
        )
        .unwrap();
    let index = builder
        .input(
            entity.variable("index").unwrap(),
            "value",
            |_| None::<usize>,
        )
        .unwrap();
    let filter = builder.entity_filter(entity, &[]).unwrap();
    assert!(!filter.requires_consistency());
    let mut session = builder.build();
    session.update(&World::default()).unwrap();
    let error = session
        .entity_is_selected(&filter, &index.erase(), Assigned)
        .unwrap_err();
    assert!(matches!(error, ShadowSessionError::Metadata(_)));
    assert!(
        error
            .to_string()
            .contains("list value has no ownership binding")
    );
}

// Adapted from pinned Timefold Community GraphStructureTest and its test-domain
// source declarations. SPDX-License-Identifier: Apache-2.0
use std::any::Any;
use timetuck::*;
include!("shadow_structure_support/fixture.rs");
fn inspect(f: Fixture, values: &[Value], owner: Option<&Owner>) -> ShadowStructure {
    let s = schema(f);
    let a = access(&s, f);
    let (vp, op) = paths(&a, f);
    let mut bindings = values
        .iter()
        .map(|v| ShadowSourceEntity {
            entity: v as &dyn Any,
            paths: vp.as_slice(),
        })
        .collect::<Vec<_>>();
    if let Some(owner) = owner {
        bindings.push(ShadowSourceEntity {
            entity: owner,
            paths: &op,
        });
    }
    ShadowStructure::inspect(bindings).unwrap()
}
fn previous() -> ShadowStructure {
    ShadowStructure::Directional(ShadowDirection::Previous)
}
#[test]
fn empty_simple_list_structure() {
    assert_eq!(inspect(Fixture::Simple, &[], None), ShadowStructure::Empty);
}
#[test]
fn simple_list_structure() {
    assert_eq!(
        inspect(Fixture::Simple, &[Value::default()], None),
        previous()
    );
}
#[test]
fn extended_simple_list_structure() {
    assert_eq!(
        inspect(Fixture::Extended, &[Value::default()], None),
        previous()
    );
}
#[test]
fn extended_simple_list_without_declarative_entities() {
    let s = schema(Fixture::Extended);
    let a = access(&s, Fixture::Extended);
    let _subclass = paths(&a, Fixture::Extended);
    assert_eq!(
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: &Base,
            paths: &[]
        }])
        .unwrap(),
        ShadowStructure::Empty
    );
}
#[test]
fn concurrent_values_without_groups() {
    assert_eq!(
        inspect(Fixture::Concurrent, &[value("v1"), value("v2")], None),
        previous()
    );
}
#[test]
fn concurrent_values_with_groups() {
    let v1 = value("v1");
    let mut v2 = value("v2");
    v2.group = vec![GroupMember::Other(Box::new(value("v1"))), GroupMember::This];
    assert_eq!(
        inspect(Fixture::Concurrent, &[v1, v2], None),
        ShadowStructure::SingleType {
            direction: Some(ShadowDirection::Previous)
        }
    );
}
#[test]
fn follower_structure() {
    assert_eq!(
        inspect(Fixture::Follower, &[Value::default()], None),
        ShadowStructure::SingleType { direction: None }
    );
}
#[test]
fn multi_entity() {
    assert_eq!(
        inspect(
            Fixture::MultiEntity,
            &[Value::default()],
            Some(&Owner::default())
        ),
        ShadowStructure::General
    );
}
#[test]
fn multi_directional_parents() {
    let v = Value {
        group: vec![GroupMember::This],
        ..Default::default()
    };
    assert_eq!(
        inspect(Fixture::MultiDirectional, &[v], Some(&Owner::default())),
        ShadowStructure::General
    );
}
#[test]
fn multi_directional_parents_empty_groups() {
    assert_eq!(
        inspect(
            Fixture::MultiDirectional,
            &[Value::default()],
            Some(&Owner::default())
        ),
        previous()
    );
}
#[test]
fn list_element_structure() {
    assert_eq!(
        inspect(
            Fixture::ListElement,
            &[value("v1")],
            Some(&Owner::default())
        ),
        ShadowStructure::General
    );
}
#[test]
fn empty_structure() {
    let _plain =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Leader>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
        ])])
        .unwrap();
    assert_eq!(
        ShadowStructure::inspect([]).unwrap(),
        ShadowStructure::Empty
    );
}

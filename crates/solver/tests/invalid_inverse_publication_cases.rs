// Adapted from Community ShadowVariableUpdateTest.invalidInverseRelation,
// Apache-2.0. See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::{Cell, RefCell};
use timetuck::*;
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Entity(&'static str);
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Value(&'static str);
struct Solution;
struct World {
    entity: Entity,
    value: Value,
    duration_seconds: i64,
    inverse: Option<Vec<Entity>>,
    facts: Vec<()>,
}
#[test]
fn invalid_inverse_relation() {
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Entity>(vec![PlanningVariableDefinition::basic::<Value>(
            "value", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![PlanningVariableDefinition::shadow::<
            Vec<Entity>,
        >("entityList")]),
    ])
    .unwrap();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("value")
        .unwrap();
    let inverse = schema
        .entity::<Value>()
        .unwrap()
        .variable("entityList")
        .unwrap();
    let world = World {
        entity: Entity("v1"),
        value: Value("c1"),
        duration_seconds: 0,
        inverse: None,
        facts: vec![],
    };
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_basic(field, [(world.entity.clone(), Some(world.value.clone()))])
        .unwrap();
    let mut supply = BasicStateSupply::new(
        registry.basic::<Entity, Value>(field).unwrap(),
        [world.value.clone()],
    )
    .unwrap();
    supply.externalize(inverse).unwrap();
    let error = supply
        .refresh_external_inverse(|_| world.inverse.as_deref())
        .unwrap_err();
    for fragment in [
        "The entity",
        "has a variable (value) with value",
        "which has a sourceVariableName variable (entityList) which is null.",
    ] {
        assert!(error.to_string().contains(fragment));
    }
    // The second source call updates a full solution. Native preflight rejects
    // the same real null field before readers, projection, publication or score.
    let supply = RefCell::new(supply);
    let calls = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, World>::new(&schema);
    builder
        .input(field, 0, |world| {
            calls.set(calls.get() + 1);
            Some(world.value.clone())
        })
        .unwrap();
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        |world, _| {
            supply
                .borrow_mut()
                .refresh_external_inverse(|_| world.inverse.as_deref())
                .map(|u| u.shadows)
        },
        |_, _| Ok(()),
        |_, _| Ok(SimpleScore::ZERO),
    )
    .and_preflight(|world| {
        supply
            .borrow()
            .validate_external_inverse(|_| world.inverse.as_deref())
    });
    let mut solution = ManagedSolution::new(world, vec![]);
    let error = manager
        .update(&mut solution)
        .expect_err("null inverse must fail");
    assert!(matches!(
        error,
        ShadowSolutionError::Adapter(BasicSupplyError::NullInverseCollection { .. })
    ));
    assert_eq!(calls.get(), 0);
    assert!(solution.assignment.inverse.is_none());
    assert_eq!(solution.assignment.duration_seconds, 0);
    assert!(solution.assignment.facts.is_empty());
    assert!(solution.shadows.is_empty());
    assert_eq!(solution.score, None);
}

//! Original live capability construction, actual moves and publication.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
enum Value {
    First,
    Second,
}
struct Entity {
    value: Value,
}
struct Solution {
    score: SimpleScore,
}
fn layout() -> Arc<ComposedPlanningSolution> {
    let e = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Value>("value", false),
    ])])
    .unwrap();
    let s = SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Solution>(
        vec![
            SolutionMemberDefinition::entity_collection::<Entity>("entities"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
    )])
    .unwrap();
    Arc::new(
        s.compile::<Solution>(
            &e,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
#[test]
fn capabilities_construct_execute_and_publish_with_immutable_values() {
    let layout = layout();
    let entity_access = ModelMutabilityAccess::mutable(
        |e: &Entity| Assignment::new(vec![Some(e.value.clone())]),
        |e, a: Assignment<Value>| e.value = a.values()[0].clone().unwrap(),
    )
    .bind(layout.clone(), PlanningModelRole::Entity)
    .unwrap();
    let solution_access =
        ModelMutabilityAccess::mutable(|s: &Solution| s.score, |s, v| s.score = v)
            .bind(layout.clone(), PlanningModelRole::Solution)
            .unwrap();
    let values = ModelMutabilityAccess::read_only(ReadOnlyModelShape::Enum, |v: &Value| {
        vec![v.clone(), Value::Second]
    })
    .bind(layout.clone(), PlanningModelRole::Value)
    .unwrap();
    assert!(Arc::ptr_eq(values.layout(), &layout));
    let mut entity = Entity {
        value: Value::First,
    };
    let mut solution = Solution {
        score: SimpleScore::ZERO,
    };
    let p = FiniteProblem::new(vec![
        Variable::new(values.read(&entity.value)).with_initial(entity.value.clone()),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("second", vec![0], |v| {
        SimpleScore::new(i64::from(v[0] == Some(&Value::Second)))
    })
    .unwrap();
    let mut director = ScoreDirector::new(&p, &n, entity_access.read(&entity)).unwrap();
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(Value::Second),
        })
        .unwrap()
        .commit();
    director.assert_consistent().unwrap();
    entity_access
        .write(&mut entity, director.assignment().clone())
        .unwrap();
    solution_access
        .write(&mut solution, director.score())
        .unwrap();
    assert_eq!(entity.value, Value::Second);
    assert_eq!(solution_access.read(&solution), SimpleScore::new(1));
    assert_eq!(entity_access.read(&entity), director.assignment().clone());
    assert!(matches!(
        values.write(&mut Value::First, vec![]),
        Err(ModelMutabilityAccessError::ReadOnly(_))
    ));
}
#[test]
fn invalid_roles_and_missing_writers_fail_before_callbacks() {
    let calls = Cell::new(0);
    struct Other;
    for role in [
        PlanningModelRole::Solution,
        PlanningModelRole::Entity,
        PlanningModelRole::Value,
    ] {
        let result = ModelMutabilityAccess::mutable(
            |_: &Other| {
                calls.set(calls.get() + 1);
            },
            |_, _| calls.set(calls.get() + 1),
        )
        .bind(layout(), role);
        assert!(matches!(
            result,
            Err(ModelMutabilityAccessError::WrongRole { .. })
        ));
    }
    for shape in [
        ReadOnlyModelShape::Native,
        ReadOnlyModelShape::Record,
        ReadOnlyModelShape::Enum,
    ] {
        let result =
            ModelMutabilityAccess::read_only(shape, |_: &Entity| calls.set(calls.get() + 1))
                .bind(layout(), PlanningModelRole::Entity);
        assert!(matches!(
            result,
            Err(ModelMutabilityAccessError::RequiresMutable { .. })
        ));
    }
    assert_eq!(calls.get(), 0);
}

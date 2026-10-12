// Original real provider invocation, alias, multiple field/type and validation controls.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
struct A {
    initial: Option<i32>,
    range: Vec<i32>,
}
struct B {
    initial: Option<i32>,
}
struct Root {
    a: Vec<Arc<A>>,
    b: Vec<Arc<B>>,
    extra: Vec<i32>,
}
fn schema() -> Arc<PlanningSchema> {
    Arc::new(
        PlanningSchema::new::<Root>(vec![
            PlanningEntityDefinition::of::<A>(vec![
                PlanningVariableDefinition::basic::<i32>("x", false),
                PlanningVariableDefinition::basic::<i32>("y", true),
            ]),
            PlanningEntityDefinition::of::<B>(vec![PlanningVariableDefinition::basic::<i32>(
                "x", false,
            )]),
        ])
        .unwrap(),
    )
}
fn fields() -> Vec<SolutionFieldRange> {
    vec![
        SolutionFieldRange::new::<A>("x", vec!["local".into(), "extra".into()]),
        SolutionFieldRange::new::<A>("y", vec!["local".into()]),
        SolutionFieldRange::new::<B>("x", vec!["local".into()]),
    ]
}
fn source() -> Root {
    let a = Arc::new(A {
        initial: Some(2),
        range: vec![1, 2],
    });
    Root {
        a: vec![a.clone(), a],
        b: vec![Arc::new(B { initial: Some(3) })],
        extra: vec![2, 4],
    }
}
#[test]
fn solution_aware_multi_provider_aliases_bind_once_and_share_current_payloads() {
    let calls = Cell::new(0);
    let mut a = SolutionEntityRangeAccessors::new(schema(), fields()).unwrap();
    a.entities(|s: &Root| {
        s.a.iter()
            .map(|e| (e.clone(), vec![("x".into(), e.initial), ("y".into(), None)]))
            .collect()
    })
    .unwrap();
    a.entities(|s: &Root| {
        s.b.iter()
            .map(|e| (e.clone(), vec![("x".into(), e.initial)]))
            .collect()
    })
    .unwrap();
    a.provider("local", |_: &Root, e: &A| {
        calls.set(calls.get() + 1);
        e.range.clone()
    })
    .unwrap();
    a.provider("extra", |s: &Root, _: &A| {
        calls.set(calls.get() + 1);
        s.extra.clone()
    })
    .unwrap();
    a.provider("local", |_: &Root, e: &B| {
        calls.set(calls.get() + 1);
        vec![e.initial.unwrap()]
    })
    .unwrap();
    let source = source();
    let bound = a.bind(&source).unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(bound.entities().len(), 2);
    assert_eq!(bound.problem().variables().len(), 3);
    let vars = bound.problem().variables();
    assert_eq!(vars[0].domain(), &[1, 2, 4]);
    assert_eq!(vars[0].initial(), Some(&2));
    assert_eq!(vars[1].domain(), &[1, 2]);
    assert!(vars[1].allows_unassigned());
    assert_eq!(vars[2].domain(), &[3]);
    assert_eq!(bound.entities()[0].slot("y"), Some(1));
    assert!(std::ptr::eq(
        bound.entities()[0].object_ref::<A>().unwrap(),
        source.a[0].as_ref()
    ));
    // A new bind reads the actual changed solution, rather than reusing stale
    // provider values across snapshots; within each bind each provider runs once.
    let mut changed = source;
    changed.extra = vec![5];
    assert_eq!(
        a.bind(&changed).unwrap().problem().variables()[0].domain(),
        &[1, 2, 5]
    );
    assert_eq!(calls.get(), 6);
}
#[test]
fn missing_bindings_and_invalid_metadata_reject_before_getters() {
    let calls = Cell::new(0);
    let mut a = SolutionEntityRangeAccessors::new(schema(), fields()).unwrap();
    a.entities(|_: &Root| {
        calls.set(calls.get() + 1);
        Vec::<BasicEntityFields<A, i32>>::new()
    })
    .unwrap();
    assert!(matches!(
        a.bind(&source()),
        Err(SolutionBindingError::MissingBinding(_))
    ));
    assert_eq!(calls.get(), 0);
    a.entities(|_: &Root| Vec::<BasicEntityFields<B, i32>>::new())
        .unwrap();
    assert!(matches!(
        a.bind(&source()),
        Err(SolutionBindingError::UnknownProvider(_))
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        a.provider("unused", |_: &Root, _: &A| vec![0]),
        Err(SolutionBindingError::UnknownProvider(_))
    ));
    let mut duplicate = fields();
    duplicate.push(duplicate[0].clone());
    assert!(matches!(
        SolutionEntityRangeAccessors::<Root, i32>::new(schema(), duplicate),
        Err(SolutionBindingError::InvalidFieldRange { .. })
    ));
    assert!(matches!(
        SolutionEntityRangeAccessors::<Root, i64>::new(schema(), fields()),
        Err(SolutionBindingError::InvalidFieldRange { .. })
    ));
}
#[test]
fn conflicting_alias_fields_reject_without_publishing_a_model() {
    let mut a = SolutionEntityRangeAccessors::new(schema(), fields()).unwrap();
    a.entities(|s: &Root| {
        vec![
            (
                s.a[0].clone(),
                vec![("x".into(), Some(2)), ("y".into(), None)],
            ),
            (
                s.a[0].clone(),
                vec![("x".into(), Some(1)), ("y".into(), None)],
            ),
        ]
    })
    .unwrap();
    a.entities(|_: &Root| Vec::<BasicEntityFields<B, i32>>::new())
        .unwrap();
    a.provider("local", |_: &Root, e: &A| e.range.clone())
        .unwrap();
    a.provider("extra", |s: &Root, _: &A| s.extra.clone())
        .unwrap();
    a.provider("local", |_: &Root, _: &B| vec![3]).unwrap();
    assert!(matches!(
        a.bind(&source()),
        Err(SolutionBindingError::ConflictingEntityRecord(_))
    ));
}

#[path = "fixtures/solution_parameter_range_fixture.rs"]
mod parameter_fixture;
#[test]
fn actual_solution_and_base_parameter_providers_bind_live_domains() {
    use parameter_fixture::{
        BaseEntity, BaseSolution, ChildEntity, Entity, ExtendedSolution, Solution, Value,
    };
    let v1 = Arc::new(Value("1"));
    let v2 = Arc::new(Value("2"));
    let v3 = Arc::new(Value("3"));
    let root = Solution {
        entities: vec![
            Arc::new(Entity {
                range: vec![v1.clone(), v2.clone()],
                value: Some(v2.clone()),
            }),
            Arc::new(Entity {
                range: vec![v1.clone(), v3.clone()],
                value: None,
            }),
        ],
    };
    let a = parameter_fixture::plain();
    let bound = a.bind(&root).unwrap();
    assert_eq!(bound.problem().variables().len(), 2);
    assert_eq!(
        bound.problem().variables()[0]
            .domain()
            .iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
    assert_eq!(
        bound.problem().variables()[1]
            .domain()
            .iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        vec!["1", "3"]
    );
    let mut root = ExtendedSolution {
        base: BaseSolution {
            values: vec![v1, v2.clone(), v3.clone()],
            entities: vec![Arc::new(ChildEntity {
                base: BaseEntity {
                    value: Some(v2.clone()),
                },
            })],
        },
    };
    let a = parameter_fixture::extended();
    let bound = a.bind(&root).unwrap();
    assert_eq!(bound.problem().variables().len(), 1);
    assert_eq!(bound.problem().variables()[0].domain().len(), 3);
    assert!(Arc::ptr_eq(
        bound.problem().variables()[0].initial().unwrap(),
        &v2
    ));
    assert!(std::ptr::eq(
        bound.entities()[0].object_ref::<ChildEntity>().unwrap(),
        root.base.entities[0].as_ref()
    ));
    root.base.values = vec![v2, v3];
    assert_eq!(
        a.bind(&root).unwrap().problem().variables()[0]
            .domain()
            .iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        vec!["2", "3"]
    );
}

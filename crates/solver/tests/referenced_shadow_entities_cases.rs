// Adapted from Timefold Community SolutionManagerTest and concurrent test fixtures.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json.
use std::{any::Any, cell::OnceCell};
use timetuck::*;
struct Solution;
struct Owner {
    id: &'static str,
    values: OnceCell<Vec<&'static Visit>>,
}
struct Visit {
    id: &'static str,
    owner: OnceCell<&'static Owner>,
    previous: OnceCell<&'static Visit>,
    next: OnceCell<&'static Visit>,
    group: OnceCell<Vec<&'static Visit>>,
}
// These six test-only objects have process lifetime to represent cyclic borrowed
// source paths through Any. Production traversal does not allocate domain objects.
fn visit(id: &'static str) -> &'static Visit {
    Box::leak(Box::new(Visit {
        id,
        owner: OnceCell::new(),
        previous: OnceCell::new(),
        next: OnceCell::new(),
        group: OnceCell::new(),
    }))
}
fn owner(id: &'static str) -> &'static Owner {
    Box::leak(Box::new(Owner {
        id,
        values: OnceCell::new(),
    }))
}
fn identify(e: &dyn Any) -> Option<&'static str> {
    e.downcast_ref::<Visit>()
        .map(|e| e.id)
        .or_else(|| e.downcast_ref::<Owner>().map(|e| e.id))
}
#[test]
fn update_shadow_variable_fails_if_referenced_entities_are_not_given() {
    let (e1, e2) = (owner("e1"), owner("e2"));
    let (a1, a2, b1, b2) = (visit("a1"), visit("a2"), visit("b1"), visit("b2"));
    for v in [a1, a2] {
        assert!(v.group.set(vec![a1, a2]).is_ok());
    }
    for v in [b1, b2] {
        assert!(v.group.set(vec![b1, b2]).is_ok());
    }
    assert!(e1.values.set(vec![a1, b1]).is_ok());
    assert!(e2.values.set(vec![b2, a2]).is_ok());
    assert!(b1.previous.set(a1).is_ok());
    assert!(a2.previous.set(b2).is_ok());
    assert!(a1.next.set(b1).is_ok());
    assert!(b2.next.set(a2).is_ok());
    for (v, e) in [(a1, e1), (b1, e1), (a2, e2), (b2, e2)] {
        assert!(v.owner.set(e).is_ok());
    }
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Visit>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Visit>(vec![
            PlanningVariableDefinition::shadow::<Owner>("entity"),
            PlanningVariableDefinition::shadow::<Visit>("previousValue"),
            PlanningVariableDefinition::shadow::<Visit>("nextValue"),
            PlanningVariableDefinition::shadow::<i64>("serviceReadyTime"),
            PlanningVariableDefinition::shadow::<i64>("serviceStartTime"),
            PlanningVariableDefinition::shadow::<i64>("serviceFinishTime"),
        ]),
    ])
    .unwrap();
    let mut registry = SourcePathRegistry::new(&schema);
    registry
        .one("entity", SourceMemberRole::Variable, |v: &Visit| {
            v.owner.get().copied()
        })
        .unwrap();
    registry
        .one("previousValue", SourceMemberRole::Previous, |v: &Visit| {
            v.previous.get().copied()
        })
        .unwrap();
    registry
        .many(
            "concurrentValueGroup",
            SourceMemberRole::Fact,
            |v: &Visit| v.group.get().cloned().unwrap_or_default(),
        )
        .unwrap();
    for field in ["serviceReadyTime", "serviceStartTime", "serviceFinishTime"] {
        registry
            .one(
                field,
                SourceMemberRole::Declarative,
                |_: &Visit| -> Option<&i64> { panic!("preflight must not read shadow values") },
            )
            .unwrap();
    }
    let paths = [
        ("serviceReadyTime", "previousValue.serviceFinishTime"),
        ("serviceReadyTime", "entity"),
        ("serviceStartTime", "serviceReadyTime"),
        (
            "serviceStartTime",
            "concurrentValueGroup[].serviceReadyTime",
        ),
        ("serviceFinishTime", "serviceStartTime"),
    ]
    .map(|(target, source)| registry.compile::<Visit>(target, source).unwrap());
    let paths = paths.iter().collect::<Vec<_>>();
    let mut builder = ShadowSessionBuilder::<&str, Vec<&dyn Any>>::new(&schema);
    let field = schema
        .entity::<Visit>()
        .unwrap()
        .variable("serviceReadyTime")
        .unwrap();
    let node = builder
        .derive(field, "a1", |_, _| -> Option<i64> {
            panic!("incomplete input must fail before refresh")
        })
        .unwrap();
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        |_, _| -> Result<(), ConsistentShadowUpdateError<&str>> {
            panic!("projection must not run")
        },
        |_, _| Ok(()),
        |_, _| -> Result<i64, _> { panic!("scoring must not run") },
    )
    .with_preflight(|entities| validate_referenced_shadow_entities(&paths, entities, identify));
    let mut solution = ManagedSolution::new(vec![a1 as &dyn Any, e1 as &dyn Any], ());
    let error = manager
        .update_with_policy(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap_err();
    let ShadowSolutionError::Adapter(ConsistentShadowUpdateError::MissingSources(missing)) = error
    else {
        panic!("expected missing-entity diagnostics: {error:?}");
    };
    // Preserve all three required diagnostic relationships from the source;
    // owned typed identities replace Java exception/message formatting.
    for (entity, field, source, source_field) in [
        ("a1", "serviceStartTime", "a2", "serviceReadyTime"),
        ("a2", "serviceReadyTime", "b2", "serviceFinishTime"),
        ("b2", "serviceStartTime", "b1", "serviceReadyTime"),
    ] {
        assert!(missing.contains(&ShadowSourceReference {
            target: ShadowVariableKey {
                entity_type: PlanningType::of::<Visit>(),
                entity,
                variable: field.into()
            },
            source: ShadowVariableKey {
                entity_type: PlanningType::of::<Visit>(),
                entity: source,
                variable: source_field.into()
            },
        }));
    }
    assert_eq!(solution.score, None);
    assert!(matches!(
        manager.session().read(&node),
        Err(ShadowSessionError::NotCurrent)
    ));
}

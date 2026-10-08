//! Original preflight resolution and preservation checks.
use std::{
    any::Any,
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
struct Solution;
#[derive(Clone, Debug)]
struct Node {
    id: usize,
    group: Vec<Arc<Node>>,
}
fn node(id: usize, group: Vec<Arc<Node>>) -> Arc<Node> {
    Arc::new(Node { id, group })
}
fn identify(value: &dyn Any) -> Option<usize> {
    value.downcast_ref::<Node>().map(|n| n.id)
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Node>(vec![
        PlanningVariableDefinition::shadow::<i64>("result"),
        PlanningVariableDefinition::shadow::<i64>("alternate"),
    ])])
    .unwrap()
}
fn paths(s: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut r = SourcePathRegistry::new(s);
    r.many("group", SourceMemberRole::Fact, |n: &Node| {
        n.group.iter().map(Arc::as_ref).collect()
    })
    .unwrap();
    for name in ["result", "alternate"] {
        r.one(
            name,
            SourceMemberRole::Declarative,
            |_: &Node| -> Option<&i64> { panic!("terminal getters are never evaluated") },
        )
        .unwrap();
    }
    r
}
#[test]
fn all_missing_fields_are_reported_once_without_running_suppliers_or_changing_cache() {
    let s = schema();
    let paths = paths(&s);
    let a = paths.compile::<Node>("result", "group[].result").unwrap();
    let b = paths
        .compile::<Node>("result", "group[].alternate")
        .unwrap();
    let result = s.entity::<Node>().unwrap().variable("result").unwrap();
    let called = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    let mut handles = vec![];
    for id in 0..2 {
        let h = builder
            .derive(result, id, |_, _| {
                called.set(called.get() + 1);
                Some(4i64)
            })
            .unwrap();
        builder
            .bind_source_paths(&h, vec![&a, &b], move |w| w[id].as_ref(), identify)
            .unwrap();
        handles.push(h);
    }
    let mut session = builder.build();
    let absent = node(7, vec![]);
    let known = node(1, vec![]);
    let bad = vec![
        node(0, vec![absent.clone(), known.clone(), absent.clone()]),
        node(1, vec![absent]),
    ];
    let report = missing_shadow_sources(&session, &bad).unwrap();
    let keys = report
        .iter()
        .map(|r| (r.target.entity, r.source.entity, r.source.variable.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        [
            (0, 7, "result"),
            (0, 7, "alternate"),
            (0, 1, "alternate"),
            (1, 7, "result"),
            (1, 7, "alternate")
        ]
    );
    for r in &report {
        assert_eq!(r.target.variable, "result");
        assert_eq!(r.source.entity_type, PlanningType::of::<Node>());
    }
    let error = update_consistent_shadows(&mut session, &bad).err().unwrap();
    assert!(error.to_string().contains("missing source fields"));
    let ConsistentShadowUpdateError::MissingSources(refs) = error else {
        panic!("wrong error")
    };
    assert_eq!(refs, report);
    assert!(
        matches!(update_shadows_with_policy(&mut session, &bad, ShadowConsistencyPolicy::AllowInconsistent),
        Err(ConsistentShadowUpdateError::MissingSources(refs)) if refs == report)
    );
    assert_eq!(called.get(), 0);
    assert!(matches!(
        session.read(&handles[0]),
        Err(ShadowSessionError::NotCurrent)
    ));
    let good = vec![node(0, vec![]), node(1, vec![])];
    assert!(missing_shadow_sources(&session, &good).unwrap().is_empty());
    session.update(&good).unwrap();
    assert_eq!(called.get(), 2);
    assert_eq!(missing_shadow_sources(&session, &bad).unwrap(), report);
    assert!(matches!(update_consistent_shadows(&mut session, &bad),
        Err(ConsistentShadowUpdateError::MissingSources(refs)) if refs == report));
    assert!(
        matches!(update_shadows_with_policy(&mut session, &bad, ShadowConsistencyPolicy::AllowInconsistent),
        Err(ConsistentShadowUpdateError::MissingSources(refs)) if refs == report)
    );
    assert_eq!(called.get(), 2);
    assert_eq!(session.read(&handles[0]).unwrap().value, Some(&4));
    session.mark_dirty(&handles[1]).unwrap();
    assert!(missing_shadow_sources(&session, &good).unwrap().is_empty());
    assert!(matches!(
        session.read(&handles[1]),
        Err(ShadowSessionError::NotCurrent)
    ));
    session.update(&good).unwrap();
    assert_eq!(called.get(), 3);
    drop(bad);
    drop(session);
    assert_eq!(report[0].source.entity, 7);
}
#[test]
fn resolver_errors_and_panics_preserve_a_ready_snapshot() {
    let s = schema();
    let paths = paths(&s);
    let path = paths.compile::<Node>("result", "group[].result").unwrap();
    let field = s.entity::<Node>().unwrap().variable("result").unwrap();
    let mut b = ShadowSessionBuilder::<usize, Arc<Node>>::new(&s);
    let h = b.derive(field, 0, |_, _| Some(99i64)).unwrap();
    b.bind_source_paths(
        &h,
        vec![&path],
        |w| w.as_ref(),
        |n| {
            let id = identify(n)?;
            assert_ne!(id, 9, "resolver panic");
            (id != 8).then_some(id)
        },
    )
    .unwrap();
    let mut session = b.build();
    let good = node(0, vec![]);
    session.update(&good).unwrap();
    assert!(matches!(
        missing_shadow_sources(&session, &node(0, vec![node(8, vec![])])),
        Err(ShadowSessionError::UnregisteredSource)
    ));
    assert!(matches!(
        missing_shadow_sources(&session, &node(1, vec![])),
        Err(ShadowSessionError::Metadata(_))
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| missing_shadow_sources(
            &session,
            &node(0, vec![node(9, vec![])])
        )))
        .is_err()
    );
    assert_eq!(session.read(&h).unwrap().value, Some(&99));
    assert!(missing_shadow_sources(&session, &good).unwrap().is_empty());
}

#[test]
fn strict_update_resolves_once_and_matches_values_and_signatures_of_ordinary_update() {
    let s = schema();
    let paths = paths(&s);
    let path = paths.compile::<Node>("result", "group[].result").unwrap();
    let field = s.entity::<Node>().unwrap().variable("result").unwrap();
    let roots = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<Arc<Node>>>::new(&s);
    let mut handles = vec![];
    for id in 0..3 {
        let h = builder
            .derive(field, id, move |_, values| {
                Some(
                    id as i64
                        + 1
                        + values
                            .for_field::<i64>(field)
                            .unwrap()
                            .into_iter()
                            .flatten()
                            .sum::<i64>(),
                )
            })
            .unwrap();
        let counter = &roots;
        builder
            .bind_source_paths(
                &h,
                vec![&path],
                move |world| {
                    counter.set(counter.get() + 1);
                    world[id].as_ref()
                },
                identify,
            )
            .unwrap();
        handles.push(h);
    }
    let mut session = builder.build();
    for step in 0..64 {
        let zero = node(0, vec![]);
        let one = node(
            1,
            if step & 1 != 0 {
                vec![zero.clone(), zero.clone()]
            } else {
                vec![]
            },
        );
        let mut group = if step & 2 != 0 {
            vec![zero.clone(), one.clone(), one.clone()]
        } else if step & 4 != 0 {
            vec![one.clone()]
        } else {
            vec![]
        };
        if step & 8 != 0 {
            group.reverse();
        }
        let world = vec![zero, one, node(2, group)];
        let before = roots.get();
        update_consistent_shadows(&mut session, &world).unwrap();
        assert_eq!(roots.get(), before + 3, "each root resolves once");
        let middle = 2 + i64::from(step & 1 != 0);
        let last = 3 + if step & 2 != 0 {
            1 + middle
        } else if step & 4 != 0 {
            middle
        } else {
            0
        };
        for (handle, expected) in handles.iter().zip([1, middle, last]) {
            assert_eq!(session.read(handle).unwrap().value, Some(&expected));
        }
        assert!(
            session.update(&world).unwrap().is_empty(),
            "ordinary update sees the same installed source signatures"
        );
        assert_eq!(roots.get(), before + 6);
    }
}

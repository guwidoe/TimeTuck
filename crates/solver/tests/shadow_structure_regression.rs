//! Original structural inspection and next-link dependency regressions.
use timetuck::*;
include!("shadow_structure_support/fixture.rs");
#[test]
fn next_links_compile_resolve_and_classify_without_reading_terminal_fields() {
    let s = schema(Fixture::Simple);
    let a = access(&s, Fixture::Simple);
    let p = [a
        .compile::<Value>("startTime", "nextValue.endTime")
        .unwrap()];
    let v = Value {
        id: "now",
        next: Some(Box::new(Value {
            id: "next",
            ..Default::default()
        })),
        ..Default::default()
    };
    assert_eq!(p[0].parent(), SourceParent::Next);
    assert_eq!(
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: &v,
            paths: &p
        }])
        .unwrap(),
        ShadowStructure::Directional(ShadowDirection::Next)
    );
    let keys = p[0]
        .dependency_keys(&v, |variable, root| {
            Some((
                root.downcast_ref::<Value>().unwrap().id,
                variable.name().to_owned(),
            ))
        })
        .unwrap();
    assert_eq!(
        keys,
        vec![("next", "endTime".into()), ("now", "nextValue".into())]
    );
}
#[test]
fn group_changes_reclassify_even_when_later_parent_links_are_null() {
    let s = schema(Fixture::MultiDirectional);
    let a = access(&s, Fixture::MultiDirectional);
    let (p, _) = paths(&a, Fixture::MultiDirectional);
    let mut v = Value::default();
    let check = |v: &Value| {
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: v,
            paths: &p,
        }])
        .unwrap()
    };
    assert_eq!(
        check(&v),
        ShadowStructure::Directional(ShadowDirection::Previous)
    );
    v.group.push(GroupMember::This);
    assert!(v.next.is_none());
    assert_eq!(check(&v), ShadowStructure::General);
    v.group.clear();
    assert_eq!(
        check(&v),
        ShadowStructure::Directional(ShadowDirection::Previous)
    );
}
#[test]
fn wrong_roots_are_rejected_even_after_general_shape_is_known() {
    let s = schema(Fixture::MultiEntity);
    let a = access(&s, Fixture::MultiEntity);
    let (vp, op) = paths(&a, Fixture::MultiEntity);
    let v = Value::default();
    let o = Owner::default();
    let error = ShadowStructure::inspect([
        ShadowSourceEntity {
            entity: &v,
            paths: &vp,
        },
        ShadowSourceEntity {
            entity: &o,
            paths: &op,
        },
        ShadowSourceEntity {
            entity: &o,
            paths: &vp,
        },
    ])
    .unwrap_err();
    assert_eq!(error.reason, "wrong traversal root type");
}
#[test]
fn local_dependencies_and_multiple_directions_are_distinguished() {
    let s = schema(Fixture::Simple);
    let a = access(&s, Fixture::Simple);
    let v = Value::default();
    let local = [a.compile::<Value>("endTime", "startTime").unwrap()];
    assert_eq!(
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: &v,
            paths: &local
        }])
        .unwrap(),
        ShadowStructure::SingleType { direction: None }
    );
    let both = [
        a.compile::<Value>("startTime", "previous.endTime").unwrap(),
        a.compile::<Value>("startTime", "nextValue.endTime")
            .unwrap(),
    ];
    assert_eq!(
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: &v,
            paths: &both
        }])
        .unwrap(),
        ShadowStructure::General
    );
}
#[test]
fn inspection_does_not_invoke_terminal_getters_or_follow_null_directional_links() {
    let s = schema(Fixture::Simple);
    let mut a = SourcePathRegistry::new(&s);
    a.one(
        "previous",
        SourceMemberRole::Previous,
        |_: &Value| -> Option<&Value> {
            panic!("structure inspection must not dereference ordinary directional links")
        },
    )
    .unwrap();
    a.one(
        "endTime",
        SourceMemberRole::Declarative,
        |_: &Value| -> Option<&i32> {
            panic!("structure inspection must not read the terminal field")
        },
    )
    .unwrap();
    let p = [a.compile::<Value>("startTime", "previous.endTime").unwrap()];
    assert_eq!(
        ShadowStructure::inspect([ShadowSourceEntity {
            entity: &Value::default(),
            paths: &p
        }])
        .unwrap(),
        ShadowStructure::Directional(ShadowDirection::Previous)
    );
}

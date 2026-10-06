// Adapted from RootVariableSourceTest and invalid declarative fixtures.
// Apache-2.0; see compat/provenance.json. Typed accessors replace reflection.
use timetuck::*;
#[derive(Default)]
struct Value {
    code: &'static str,
    fact: Option<Box<Value>>,
    group: Vec<Value>,
    previous: Option<Box<Value>>,
    dependency: Option<Box<Value>>,
    shadow: Option<Box<Value>>,
    inconsistent: bool,
}
#[derive(Default)]
struct Entity {
    fact: Option<Box<Entity>>,
    values: Vec<Value>,
    shadow: i32,
}
struct Solution;
fn value(code: &'static str) -> Value {
    Value {
        code,
        ..Default::default()
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<Value>("dependency"),
            PlanningVariableDefinition::shadow::<Value>("shadow"),
        ]),
        PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::list::<Value>("values", false),
            PlanningVariableDefinition::shadow::<i32>("shadow"),
        ]),
    ])
    .unwrap()
}
fn access(s: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut a = SourcePathRegistry::new(s);
    a.one("fact", SourceMemberRole::Fact, |v: &Value| {
        v.fact.as_deref()
    })
    .unwrap();
    a.many("group", SourceMemberRole::Fact, |v: &Value| {
        v.group.iter().collect()
    })
    .unwrap();
    a.one("previous", SourceMemberRole::Previous, |v: &Value| {
        v.previous.as_deref()
    })
    .unwrap();
    a.one("dependency", SourceMemberRole::Declarative, |v: &Value| {
        v.dependency.as_deref()
    })
    .unwrap();
    a.one("shadow", SourceMemberRole::Declarative, |v: &Value| {
        v.shadow.as_deref()
    })
    .unwrap();
    a.one(
        "inconsistent",
        SourceMemberRole::Inconsistent,
        |v: &Value| Some(&v.inconsistent),
    )
    .unwrap();
    a.one("fact", SourceMemberRole::Fact, |v: &Entity| {
        v.fact.as_deref()
    })
    .unwrap();
    a.many("values", SourceMemberRole::Variable, |v: &Entity| {
        v.values.iter().collect()
    })
    .unwrap();
    a.one("shadow", SourceMemberRole::Declarative, |v: &Entity| {
        Some(&v.shadow)
    })
    .unwrap();
    a
}
fn check(
    path: &str,
    parent: SourceParent,
    root: &Value,
    expected: &[&Value],
    names: &[&str],
    chains: &[&[&str]],
) {
    let s = schema();
    let a = access(&s);
    let p = a.compile::<Value>("shadow", path).unwrap();
    assert_eq!(p.root_type(), PlanningType::of::<Value>());
    assert_eq!(p.parent(), parent);
    assert!(p.list_variable().is_none());
    let e = s.entity::<Value>().unwrap();
    let target = e.variable("shadow").unwrap();
    assert_eq!(p.target(), target);
    let refs = p.references().collect::<Vec<_>>();
    assert_eq!(refs.len(), names.len());
    for (i, r) in refs.iter().copied().enumerate() {
        assert_eq!(r.variable(), e.variable(names[i]).unwrap());
        assert_eq!(r.target(), target);
        assert_eq!(r.chain(), chains[i]);
        assert_eq!(r.is_top_level(), i == 0);
        assert_eq!(r.is_bottom_level(), i + 1 == names.len());
        assert_eq!(
            r.on_root(),
            i == 0 && !path.starts_with("group") && !path.starts_with("fact")
        );
        assert_eq!(r.is_declarative(), names[i] == "dependency");
        assert_eq!(
            r.downstream_declarative(),
            names
                .contains(&"dependency")
                .then(|| e.variable("dependency").unwrap())
        );
        for entity in r.entities(root).unwrap() {
            let mapped = r.target_entities_from_variable(entity).unwrap();
            assert_eq!(mapped.len(), 1);
            assert!(mapped.iter().all(|v| {
                expected
                    .iter()
                    .any(|x| std::ptr::eq(*x, v.downcast_ref::<Value>().unwrap()))
            }));
        }
    }
    let visited = p.value_entities(root).unwrap();
    assert_eq!(visited.len(), expected.len());
    for (actual, expected) in visited.iter().zip(expected) {
        assert!(std::ptr::eq(
            actual.downcast_ref::<Value>().unwrap(),
            *expected
        ));
    }
    // Terminal source references map their own entity identically, including a group root.
    let end = refs
        .last()
        .unwrap()
        .target_entities_from_variable(root)
        .unwrap();
    assert_eq!(end.len(), 1);
    assert!(std::ptr::eq(end[0].downcast_ref::<Value>().unwrap(), root));
}
#[test]
fn builtin() {
    let v = value("v1");
    check(
        "previous",
        SourceParent::None,
        &v,
        &[&v],
        &["previous"],
        &[&[]],
    );
}
#[test]
fn declarative() {
    let v = value("v1");
    check(
        "dependency",
        SourceParent::None,
        &v,
        &[&v],
        &["dependency"],
        &[&[]],
    );
}
#[test]
fn declarative_group() {
    let v = Value {
        group: vec![value("v1"), value("v2")],
        ..value("group")
    };
    check(
        "group[].dependency",
        SourceParent::Group,
        &v,
        &[&v.group[0], &v.group[1]],
        &["dependency"],
        &[&[]],
    );
}
#[test]
fn builtin_group() {
    let v = Value {
        group: vec![value("v1"), value("v2")],
        ..value("group")
    };
    check(
        "group[].previous",
        SourceParent::Group,
        &v,
        &[&v.group[0], &v.group[1]],
        &["previous"],
        &[&[]],
    );
}
#[test]
fn declarative_group_fact() {
    let v = Value {
        fact: Some(Box::new(Value {
            group: vec![value("v1"), value("v2")],
            ..value("fact")
        })),
        ..value("root")
    };
    let f = v.fact.as_ref().unwrap();
    check(
        "fact.group[].dependency",
        SourceParent::Group,
        &v,
        &[&f.group[0], &f.group[1]],
        &["dependency"],
        &[&[]],
    );
}
#[test]
fn declarative_previous() {
    let v = Value {
        previous: Some(Box::new(value("previous"))),
        ..value("current")
    };
    check(
        "previous.dependency",
        SourceParent::Previous,
        &v,
        &[v.previous.as_deref().unwrap()],
        &["previous", "dependency"],
        &[&[], &["previous"]],
    );
}
#[test]
fn builtin_fact() {
    let v = Value {
        fact: Some(Box::new(Value {
            previous: Some(Box::new(value("previous"))),
            ..value("fact")
        })),
        ..value("current")
    };
    check(
        "fact.previous",
        SourceParent::Indirect,
        &v,
        &[v.fact.as_deref().unwrap()],
        &["previous"],
        &[&["fact"]],
    );
}
#[test]
fn declarative_previous_group() {
    let v = Value {
        group: vec![Value {
            previous: Some(Box::new(value("previous"))),
            ..value("current")
        }],
        ..value("group")
    };
    check(
        "group[].previous.dependency",
        SourceParent::Group,
        &v,
        &[v.group[0].previous.as_deref().unwrap()],
        &["previous", "dependency"],
        &[&[], &["previous"]],
    );
}
#[test]
fn builtin_previous() {
    let v = Value {
        previous: Some(Box::new(value("previous"))),
        ..value("current")
    };
    check(
        "previous.previous",
        SourceParent::Previous,
        &v,
        &[v.previous.as_deref().unwrap()],
        &["previous", "previous"],
        &[&[], &["previous"]],
    );
}
#[test]
fn builtin_previous_fact() {
    let v = Value {
        fact: Some(Box::new(Value {
            previous: Some(Box::new(value("previous"))),
            ..value("fact")
        })),
        ..value("current")
    };
    check(
        "fact.previous.previous",
        SourceParent::Indirect,
        &v,
        &[v.fact.as_ref().unwrap().previous.as_deref().unwrap()],
        &["previous", "previous"],
        &[&["fact"], &["fact", "previous"]],
    );
}
#[test]
fn declarative_list() {
    let s = schema();
    let a = access(&s);
    let p = a.compile::<Entity>("shadow", "values[].shadow").unwrap();
    let e = s.entity::<Entity>().unwrap();
    let v = s.entity::<Value>().unwrap();
    assert_eq!(p.root_type(), PlanningType::of::<Entity>());
    assert_eq!(p.parent(), SourceParent::ListElement);
    assert_eq!(p.list_variable(), Some(e.variable("values").unwrap()));
    let refs = p.references().collect::<Vec<_>>();
    assert_eq!(refs.len(), 1);
    let r = refs[0];
    assert_eq!(r.variable(), v.variable("shadow").unwrap());
    assert_eq!(r.target(), e.variable("shadow").unwrap());
    assert!(r.is_top_level());
    assert!(r.is_bottom_level());
    assert!(!r.on_root());
    assert!(r.is_declarative());
    assert!(r.chain().is_empty());
    assert_eq!(
        r.downstream_declarative(),
        Some(v.variable("shadow").unwrap())
    );
    let root = Entity {
        values: vec![value("v1"), value("v2")],
        ..Default::default()
    };
    let visited = p.value_entities(&root).unwrap();
    assert_eq!(visited.len(), 2);
    for (actual, expected) in visited.iter().zip(&root.values) {
        assert!(std::ptr::eq(
            actual.downcast_ref::<Value>().unwrap(),
            expected
        ));
    }
    assert_eq!(root.values[0].code, "v1");
    assert_eq!(root.values[1].code, "v2");
}
fn invalid_value(path: &str, reason: &str) {
    let s = schema();
    let a = access(&s);
    let err = a.compile::<Value>("shadow", path).err().unwrap();
    assert_eq!(err.path, path);
    assert!(err.to_string().contains(reason), "{err}");
    assert!(err.to_string().contains("Value"));
}
fn invalid_entity(path: &str, reason: &str) {
    let s = schema();
    let a = access(&s);
    let err = a.compile::<Entity>("shadow", path).err().unwrap();
    assert_eq!(err.path, path);
    assert!(err.to_string().contains(reason), "{err}");
    assert!(err.to_string().contains("Entity") || err.to_string().contains("Value"));
}
#[test]
fn missing_property() {
    invalid_value("missing", "member does not exist");
}
#[test]
fn too_many_variables() {
    invalid_value("previous.previous.previous", "maximum of 2");
}
#[test]
fn indirect_after_variable() {
    invalid_value(
        "previous.fact.previous",
        "non-declarative variable accessed via a fact",
    );
}
#[test]
fn group_after_group() {
    invalid_value(
        "group[].group[].previous",
        "collection after another collection",
    );
}
#[test]
fn bare_list() {
    invalid_entity("values", "collections require []");
}
#[test]
fn builtin_after_list() {
    invalid_entity("values[].previous", "require a declarative shadow");
}
#[test]
fn list_after_fact() {
    invalid_entity("fact.values[].shadow", "via another member");
}
#[test]
fn declarative_after_declarative() {
    invalid_value("dependency.dependency", "after a declarative shadow");
}
#[test]
fn no_variables() {
    invalid_value("fact", "does not reference any variables");
}
#[test]
fn end_on_fact() {
    invalid_value("previous.fact", "does not end on a variable");
}
#[test]
fn multiple_facts() {
    invalid_value("fact.fact.dependency", "multiple facts in a row");
}
#[test]
fn consistency_source() {
    invalid_value("inconsistent", "consistency properties cannot be sources");
}

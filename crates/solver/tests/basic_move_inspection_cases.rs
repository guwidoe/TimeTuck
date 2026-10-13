// Adapted from pinned Community selector move getters/descriptions and ChangeMoveTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
type Move<'s> = InspectedBasicMove<'s, &'static str, &'static str>;
fn schema(multi: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<&'static str>(
        (if multi {
            vec![
                "primaryValue",
                "secondaryValue",
                "tertiaryValueAllowedUnassigned",
            ]
        } else {
            vec!["value"]
        })
        .into_iter()
        .map(|name| PlanningVariableDefinition::basic::<&'static str>(name, true))
        .collect(),
    )])
    .unwrap()
}
fn field<'s>(schema: &'s PlanningSchema, name: &str) -> PlanningVariableRef<'s> {
    schema
        .entity::<&'static str>()
        .unwrap()
        .variable(name)
        .unwrap()
}
fn setup<'s>(schema: &'s PlanningSchema, multi: bool) -> VariableStateRegistry<'s> {
    let mut registry = VariableStateRegistry::new(schema);
    if multi {
        for (name, b, c) in [
            ("primaryValue", "v1", "v2"),
            ("secondaryValue", "v3", "v4"),
            ("tertiaryValueAllowedUnassigned", "w1", "w2"),
        ] {
            registry
                .reset_basic(
                    field(schema, name),
                    [
                        ("a", None),
                        ("b", Some(b)),
                        ("c", Some(c)),
                        ("d", None),
                        ("e", None),
                        ("f", None),
                        ("g", None),
                        ("h", None),
                    ],
                )
                .unwrap();
        }
    } else {
        registry
            .reset_basic(
                field(schema, "value"),
                [("a", None), ("b", Some("v1")), ("c", Some("v2"))],
            )
            .unwrap();
    }
    registry
}
fn unassigned_registry(s: &PlanningSchema) -> VariableStateRegistry<'_> {
    let mut r = VariableStateRegistry::new(s);
    for name in [
        "primaryValue",
        "secondaryValue",
        "tertiaryValueAllowedUnassigned",
    ] {
        r.reset_basic(
            field(s, name),
            ["a", "b", "c", "d", "e", "f", "g", "h"].map(|e| (e, None::<&str>)),
        )
        .unwrap();
    }
    r
}
fn description(m: &Move<'_>) -> String {
    m.describe(|e| (*e).into(), |v| (*v).into())
}
#[test]
fn change_getters() {
    let s = schema(true);
    let mut r = unassigned_registry(&s);
    for (name, entity, to) in [
        ("primaryValue", "a", None),
        ("secondaryValue", "b", Some("1")),
    ] {
        let m = Move::change(&mut r, field(&s, name), entity, to).unwrap();
        assert_eq!(m.left_entities(), &[entity]);
        assert_eq!(m.variables()[0].name(), name);
        assert_eq!(m.change_destination(), Some(&to));
    }
}
#[test]
fn change_to_string() {
    let s = schema(false);
    let mut r = setup(&s, false);
    for (e, to, expected) in [
        ("a", None, "a {null -> null}"),
        ("a", Some("v1"), "a {null -> v1}"),
        ("a", Some("v2"), "a {null -> v2}"),
        ("b", None, "b {v1 -> null}"),
        ("b", Some("v1"), "b {v1 -> v1}"),
        ("b", Some("v2"), "b {v1 -> v2}"),
    ] {
        assert_eq!(
            description(&Move::change(&mut r, field(&s, "value"), e, to).unwrap()),
            expected
        );
    }
}
#[test]
fn change_to_string_multi_var() {
    let s = schema(true);
    let mut r = setup(&s, true);
    for (e, to, expected) in [
        ("a", None, "a {null -> null}"),
        ("a", Some("v1"), "a {null -> v1}"),
        ("a", Some("v2"), "a {null -> v2}"),
        ("b", None, "b {v3 -> null}"),
        ("b", Some("v1"), "b {v3 -> v1}"),
        ("b", Some("v2"), "b {v3 -> v2}"),
        ("c", Some("v3"), "c {v4 -> v3}"),
    ] {
        assert_eq!(
            description(&Move::change(&mut r, field(&s, "secondaryValue"), e, to).unwrap()),
            expected
        );
    }
}
#[test]
fn swap_getters() {
    let s = schema(true);
    let mut r = unassigned_registry(&s);
    for (names, left, right) in [
        (vec!["primaryValue"], "a", "b"),
        (vec!["primaryValue", "secondaryValue"], "c", "d"),
    ] {
        let m = Move::swap(
            &mut r,
            names.iter().map(|n| field(&s, n)).collect(),
            left,
            right,
        )
        .unwrap();
        assert_eq!(m.left_entities(), &[left]);
        assert_eq!(m.right_entities(), &[right]);
    }
}
#[test]
fn swap_to_string() {
    let s = schema(false);
    let mut r = setup(&s, false);
    for (l, rgt, expected) in [
        ("a", "a", "a {null} <-> a {null}"),
        ("a", "b", "a {null} <-> b {v1}"),
        ("a", "c", "a {null} <-> c {v2}"),
        ("b", "c", "b {v1} <-> c {v2}"),
        ("c", "b", "c {v2} <-> b {v1}"),
    ] {
        assert_eq!(
            description(&Move::swap(&mut r, vec![field(&s, "value")], l, rgt).unwrap()),
            expected
        );
    }
}
#[test]
fn swap_to_string_multi_var() {
    let s = schema(true);
    let mut r = setup(&s, true);
    let fields = || {
        [
            "primaryValue",
            "secondaryValue",
            "tertiaryValueAllowedUnassigned",
        ]
        .map(|n| field(&s, n))
        .to_vec()
    };
    for (l, rgt, expected) in [
        ("a", "a", "a {null, null, null} <-> a {null, null, null}"),
        ("a", "b", "a {null, null, null} <-> b {v1, v3, w1}"),
        ("a", "c", "a {null, null, null} <-> c {v2, v4, w2}"),
        ("b", "c", "b {v1, v3, w1} <-> c {v2, v4, w2}"),
        ("c", "b", "c {v2, v4, w2} <-> b {v1, v3, w1}"),
    ] {
        assert_eq!(
            description(&Move::swap(&mut r, fields(), l, rgt).unwrap()),
            expected
        );
    }
}
fn pillars<'s>(s: &'s PlanningSchema) -> VariableStateRegistry<'s> {
    let mut r = VariableStateRegistry::new(s);
    r.reset_basic(
        field(s, "value"),
        [
            ("a", None),
            ("b", None),
            ("c", Some("v1")),
            ("d", Some("v1")),
            ("e", Some("v1")),
            ("f", Some("v2")),
            ("g", Some("v2")),
        ],
    )
    .unwrap();
    r
}
#[test]
fn pillar_change_getters() {
    let s = schema(true);
    let mut r = unassigned_registry(&s);
    // Getter source fixtures start every entity unassigned.
    for name in ["primaryValue", "secondaryValue"] {
        r.reset_basic(
            field(&s, name),
            ["a", "b", "c", "d"].map(|e| (e, None::<&str>)),
        )
        .unwrap();
    }
    for (name, entities, to) in [
        ("primaryValue", vec!["a", "b"], None),
        ("secondaryValue", vec!["c", "d"], Some("1")),
    ] {
        let m = Move::pillar_change(&mut r, field(&s, name), entities.clone(), to).unwrap();
        assert_eq!(m.left_entities(), entities);
        assert_eq!(m.variables()[0].name(), name);
        assert_eq!(m.change_destination(), Some(&to));
    }
}
#[test]
fn pillar_change_to_string() {
    let s = schema(false);
    let mut r = pillars(&s);
    for (entities, to, expected) in [
        (vec!["a", "b"], Some("v1"), "[a, b] {null -> v1}"),
        (vec!["a", "b"], Some("v2"), "[a, b] {null -> v2}"),
        (vec!["c", "d", "e"], None, "[c, d, e] {v1 -> null}"),
        (vec!["c", "d", "e"], Some("v2"), "[c, d, e] {v1 -> v2}"),
        (vec!["d"], Some("v2"), "[d] {v1 -> v2}"),
    ] {
        assert_eq!(
            description(&Move::pillar_change(&mut r, field(&s, "value"), entities, to).unwrap()),
            expected
        );
    }
}
#[test]
fn pillar_swap_getters() {
    let s = schema(true);
    let mut r = unassigned_registry(&s);
    for name in ["primaryValue", "secondaryValue"] {
        r.reset_basic(
            field(&s, name),
            ["a", "b", "c", "d", "e", "f", "g", "h"].map(|e| (e, None::<&str>)),
        )
        .unwrap();
    }
    for (names, left, right) in [
        (vec!["primaryValue"], vec!["a", "b"], vec!["c", "d"]),
        (
            vec!["primaryValue", "secondaryValue"],
            vec!["e", "f"],
            vec!["g", "h"],
        ),
    ] {
        let m = Move::pillar_swap(
            &mut r,
            names.iter().map(|n| field(&s, n)).collect(),
            left.clone(),
            right.clone(),
        )
        .unwrap();
        assert_eq!(
            m.variables().iter().map(|v| v.name()).collect::<Vec<_>>(),
            names
        );
        assert_eq!(m.left_entities(), left);
        assert_eq!(m.right_entities(), right);
    }
}
#[test]
fn pillar_swap_to_string() {
    let s = schema(false);
    let mut r = pillars(&s);
    for (left, right, expected) in [
        (
            vec!["a", "b"],
            vec!["c", "d", "e"],
            "[a, b] {null} <-> [c, d, e] {v1}",
        ),
        (vec!["b"], vec!["c"], "[b] {null} <-> [c] {v1}"),
        (
            vec!["f", "g"],
            vec!["c", "d", "e"],
            "[f, g] {v2} <-> [c, d, e] {v1}",
        ),
    ] {
        assert_eq!(
            description(&Move::pillar_swap(&mut r, vec![field(&s, "value")], left, right).unwrap()),
            expected
        );
    }
}
#[test]
fn original_null_value_survives_assignment() {
    let s = schema(false);
    let mut r = VariableStateRegistry::new(&s);
    let v = field(&s, "value");
    // Generated source fixture: two available values and one initially unassigned entity.
    let available = ["value0", "value1"];
    r.reset_basic(v, [("entity0", None::<&str>)]).unwrap();
    let m = Move::change(&mut r, v, "entity0", Some(available[0])).unwrap();
    m.execute(&mut r).unwrap();
    assert!(description(&m).contains(&format!("null -> {}", available[0])));
    assert_eq!(
        r.basic::<&str, &str>(v).unwrap().snapshot()["entity0"],
        Some(available[0])
    );
}

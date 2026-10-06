// Adapted from BasicVariableStateTest and basic/inverse relation test fixtures.
// Apache-2.0; see compat/provenance.json. Explicit metadata/events replace JVM setters/mocks.
use timetuck::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Entity(&'static str);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Value(&'static str);
struct Solution;
fn schema(inverse: bool) -> PlanningSchema {
    let mut entities = vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<Value>("value", inverse),
    ])];
    if inverse {
        entities.push(PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Vec<Entity>>("entities"),
        ]));
    }
    PlanningSchema::new::<Solution>(entities).unwrap()
}
fn setup(
    s: &PlanningSchema,
) -> (
    VariableStateRegistry<'_>,
    BasicStateSupply<'_, Entity, Value>,
) {
    let field = s.entity::<Entity>().unwrap().variable("value").unwrap();
    let mut r = VariableStateRegistry::new(s);
    r.reset_basic(
        field,
        [
            (Entity("a"), Some(Value("1"))),
            (Entity("b"), Some(Value("1"))),
            (Entity("c"), Some(Value("3"))),
            (Entity("d"), Some(Value("3"))),
        ],
    )
    .unwrap();
    let supply = BasicStateSupply::new(
        r.basic::<Entity, Value>(field).unwrap(),
        [Value("1"), Value("2"), Value("3")],
    )
    .unwrap();
    (r, supply)
}
#[test]
fn externalized_mode() {
    let s = schema(true);
    let (mut r, mut supply) = setup(&s);
    supply
        .externalize(s.entity::<Value>().unwrap().variable("entities").unwrap())
        .unwrap();
    let mut table = std::collections::BTreeMap::from([
        (Value("1"), vec![Entity("a"), Entity("b")]),
        (Value("2"), vec![]),
        (Value("3"), vec![Entity("c"), Entity("d")]),
    ]);
    supply.seed_inverse(table.clone()).unwrap();
    assert!(supply.refresh_changed().unwrap().shadows.is_empty());
    assert_eq!(table[&Value("1")], [Entity("a"), Entity("b")]);
    assert!(table[&Value("2")].is_empty());
    assert_eq!(table[&Value("3")], [Entity("c"), Entity("d")]);
    for change in supply
        .change(&mut r, Entity("c"), Some(Value("2")))
        .unwrap()
        .shadows
    {
        assert_eq!(table[&change.value], change.before);
        table.insert(change.value, change.after);
    }
    assert_eq!(table[&Value("1")], [Entity("a"), Entity("b")]);
    assert_eq!(table[&Value("2")], [Entity("c")]);
    assert_eq!(table[&Value("3")], [Entity("d")]);
}
#[test]
fn map_mode() {
    let s = schema(false);
    let (mut r, mut supply) = setup(&s);
    let update = supply.refresh_changed().unwrap();
    assert_eq!(supply.inverse(&Value("1")), [Entity("a"), Entity("b")]);
    assert!(supply.inverse(&Value("2")).is_empty());
    assert_eq!(supply.inverse(&Value("3")), [Entity("c"), Entity("d")]);
    assert_eq!(
        update.notifications,
        [Value("1"), Value("1"), Value("3"), Value("3")]
    );
    let update = supply
        .change(&mut r, Entity("c"), Some(Value("2")))
        .unwrap();
    assert_eq!(supply.inverse(&Value("1")), [Entity("a"), Entity("b")]);
    assert_eq!(supply.inverse(&Value("2")), [Entity("c")]);
    assert_eq!(supply.inverse(&Value("3")), [Entity("d")]);
    assert_eq!(update.notifications, [Value("3"), Value("2")]);
    drop(supply);
}
#[test]
fn cannot_externalize_twice() {
    let s = schema(true);
    let (_, mut supply) = setup(&s);
    let field = s.entity::<Value>().unwrap().variable("entities").unwrap();
    supply.externalize(field).unwrap();
    assert!(matches!(
        supply.externalize(field),
        Err(BasicSupplyError::Metadata(_))
    ));
}

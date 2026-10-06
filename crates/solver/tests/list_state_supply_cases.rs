// Adapted from DefaultListVariableStateTest and basic/optional/pinned list fixtures.
// Apache-2.0; see compat/provenance.json. Typed outward records replace Java setters/notifier mocks.
use timetuck::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Value(&'static str);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Owner(&'static str);
struct Solution;
#[derive(Clone, Copy)]
enum Mode {
    Basic,
    Optional,
    Pinned,
}
fn schema(mode: Mode) -> PlanningSchema {
    let mut fields = vec![PlanningVariableDefinition::shadow::<Owner>("entity")];
    if !matches!(mode, Mode::Pinned) {
        fields.push(PlanningVariableDefinition::shadow::<usize>("index"));
    }
    if matches!(mode, Mode::Optional) {
        fields.extend([
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<Value>("next"),
        ]);
    }
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "valueList",
            matches!(mode, Mode::Optional),
        )]),
        PlanningEntityDefinition::of::<Value>(fields),
    ])
    .unwrap()
}
fn setup(
    s: &PlanningSchema,
    rows: Vec<(Owner, Vec<Value>)>,
    values: Vec<Value>,
) -> (VariableStateRegistry<'_>, ListStateSupply<'_, Owner, Value>) {
    let field = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
    let mut registry = VariableStateRegistry::new(s);
    registry.reset_lists(field, rows).unwrap();
    let supply =
        ListStateSupply::new(registry.list::<Owner, Value>(field).unwrap(), values).unwrap();
    (registry, supply)
}
fn bind_declared<'s>(s: &'s PlanningSchema, supply: &mut ListStateSupply<'s, Owner, Value>) {
    let e = s.entity::<Value>().unwrap();
    for (kind, name) in [
        (ListShadowProperty::Owner, "entity"),
        (ListShadowProperty::Index, "index"),
        (ListShadowProperty::Previous, "previous"),
        (ListShadowProperty::Next, "next"),
    ] {
        if let Ok(v) = e.variable(name) {
            supply.externalize(kind, v).unwrap();
        }
    }
}
fn data() -> (Owner, Owner, Value, Value, Value) {
    (Owner("e1"), Owner("e2"), Value("1"), Value("2"), Value("3"))
}
#[test]
fn initialize_round_trip() {
    let s = schema(Mode::Optional);
    let (e1, e2, v1, v2, v3) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1]), (e2, vec![])], vec![v1, v2, v3]);
    let update = supply.refresh_all().unwrap();
    assert_eq!(supply.unassigned_count(), 2);
    assert!(supply.is_assigned(&v1));
    assert!(!supply.is_assigned(&v2));
    assert!(!supply.is_assigned(&v3));
    assert_eq!(update.notifications, [v1]);
    assert!(update.shadows.is_empty());
}
#[test]
fn assign_round_trip() {
    let s = schema(Mode::Optional);
    let (e1, e2, v1, v2, v3) = data();
    let (mut r, mut supply) = setup(&s, vec![(e1, vec![v1]), (e2, vec![])], vec![v1, v2, v3]);
    assert_eq!(supply.refresh_all().unwrap().notifications, [v1]);
    assert_eq!(supply.unassigned_count(), 2);
    let position = supply.location(&v1).unwrap();
    assert_eq!((position.entity, position.index), (e1, 0));
    assert!(supply.location(&v2).is_none());
    assert!(supply.location(&v3).is_none());
    let update = supply.unassign(&mut r, &v1).unwrap();
    assert_eq!(supply.unassigned_count(), 3);
    assert!(supply.location(&v1).is_none());
    assert!(supply.location(&v2).is_none());
    assert!(supply.location(&v3).is_none());
    assert_eq!(update.notifications, [v1]);
    assert!(matches!(
        supply.unassign(&mut r, &v1),
        Err(ListSupplyError::AlreadyUnassigned(_))
    ));
}
#[test]
fn full_shadows_do_not_renotify_unchanged_elements() {
    let s = schema(Mode::Optional);
    let (e1, _, v1, v2, v3) = data();
    let (mut r, mut supply) = setup(&s, vec![(e1, vec![v1, v2, v3])], vec![v1, v2, v3]);
    bind_declared(&s, &mut supply);
    supply.refresh_all().unwrap();
    r.update_lists(supply.source(), [(e1, vec![v2, v3])])
        .unwrap();
    let update = supply.refresh_range(&e1, 0, 0).unwrap();
    assert_eq!(supply.shadow_values(&v2).unwrap().index, Some(0));
    assert_eq!(supply.shadow_values(&v3).unwrap().index, Some(1));
    assert!(update.notifications.is_empty());
    let repeated = supply.refresh_range(&e1, 0, 0).unwrap();
    assert!(repeated.notifications.is_empty());
    assert!(repeated.shadows.is_empty());
}
#[test]
fn full_shadows_do_not_notify_consistent_initial_values() {
    let s = schema(Mode::Optional);
    let (e1, _, v1, v2, _) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1, v2])], vec![v1, v2]);
    bind_declared(&s, &mut supply);
    supply
        .seed_shadows([
            (
                v1,
                ListBuiltinValues {
                    owner: Some(e1),
                    index: Some(0),
                    previous: None,
                    next: Some(v2),
                },
            ),
            (
                v2,
                ListBuiltinValues {
                    owner: Some(e1),
                    index: Some(1),
                    previous: Some(v1),
                    next: None,
                },
            ),
        ])
        .unwrap();
    let update = supply.refresh_all().unwrap();
    assert!(update.notifications.is_empty());
    assert!(update.shadows.is_empty());
}
#[test]
fn partial_shadows_notify_unchanged_rescanned_element() {
    let s = schema(Mode::Basic);
    let (e1, _, v1, v2, v3) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1, v2, v3])], vec![v1, v2, v3]);
    bind_declared(&s, &mut supply);
    supply.refresh_all().unwrap();
    let update = supply.refresh_range(&e1, 2, 2).unwrap();
    assert_eq!(update.notifications, [v3]);
    assert!(update.shadows.is_empty());
}
#[test]
fn pin_index_boundary() {
    let s = schema(Mode::Pinned);
    let (e1, _, v1, v2, v3) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1, v2, v3])], vec![v1, v2, v3]);
    supply.set_pin(
        e1,
        ListPinPolicy {
            fully_pinned: false,
            prefix: 2,
        },
    );
    assert!(supply.is_pinned(&v1));
    assert!(supply.is_pinned(&v2));
    assert!(!supply.is_pinned(&v3));
}
#[test]
fn whole_entity_is_pinned() {
    let s = schema(Mode::Pinned);
    let (e1, _, v1, v2, _) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1, v2])], vec![v1, v2]);
    supply.set_pin(
        e1,
        ListPinPolicy {
            fully_pinned: true,
            prefix: 0,
        },
    );
    assert!(supply.is_pinned(&v1));
    assert!(supply.is_pinned(&v2));
}
#[test]
fn unassigned_element_is_unpinned() {
    let s = schema(Mode::Pinned);
    let (e1, _, v1, v2, _) = data();
    let (_, mut supply) = setup(&s, vec![(e1, vec![v1])], vec![v1, v2]);
    supply.set_pin(
        e1,
        ListPinPolicy {
            fully_pinned: false,
            prefix: 1,
        },
    );
    assert!(!supply.is_assigned(&v2));
    assert!(!supply.is_pinned(&v2));
    assert!(supply.is_pinned(&v1));
}
#[test]
fn unsupported_pinning_needs_no_working_solution() {
    let s = schema(Mode::Basic);
    let (e1, _, v1, _, _) = data();
    let (mut r, supply) = setup(&s, vec![], vec![v1]);
    assert!(!supply.is_pinned(&v1));
    r.reset_lists(supply.source(), [(e1, vec![v1])]).unwrap();
    assert!(!supply.is_pinned(&v1));
}
#[test]
fn pinned_status_follows_current_position() {
    let s = schema(Mode::Pinned);
    let (e1, _, v1, v2, _) = data();
    let (mut r, mut supply) = setup(&s, vec![(e1, vec![v1, v2])], vec![v1, v2]);
    supply.set_pin(
        e1,
        ListPinPolicy {
            fully_pinned: false,
            prefix: 1,
        },
    );
    assert!(supply.is_pinned(&v1));
    assert!(!supply.is_pinned(&v2));
    r.update_lists(supply.source(), [(e1, vec![v2, v1])])
        .unwrap();
    supply.refresh_range(&e1, 0, 2).unwrap();
    assert!(!supply.is_pinned(&v1));
    assert!(supply.is_pinned(&v2));
}

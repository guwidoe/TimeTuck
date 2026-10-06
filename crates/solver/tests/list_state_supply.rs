//! Original native model, shadow publication and deferred-supplier integration.
use std::{cell::Cell, collections::BTreeMap};
use timetuck::*;
struct Solution;
struct Route;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<u32>(
            "visits", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![
            PlanningVariableDefinition::shadow::<usize>("owner"),
            PlanningVariableDefinition::shadow::<usize>("index"),
            PlanningVariableDefinition::shadow::<u32>("previous"),
            PlanningVariableDefinition::shadow::<u32>("next"),
            PlanningVariableDefinition::shadow::<i64>("arrival"),
        ]),
    ])
    .unwrap()
}
fn bind<'s>(s: &'s PlanningSchema, supply: &mut ListStateSupply<'s, usize, u32>) {
    let e = s.entity::<u32>().unwrap();
    for (property, name) in [
        (ListShadowProperty::Owner, "owner"),
        (ListShadowProperty::Index, "index"),
        (ListShadowProperty::Previous, "previous"),
        (ListShadowProperty::Next, "next"),
    ] {
        supply
            .externalize(property, e.variable(name).unwrap())
            .unwrap();
    }
}
fn consume<'s>(
    update: ListStateUpdate<'s, usize, u32>,
    table: &mut BTreeMap<u32, ListBuiltinValues<usize, u32>>,
    session: &mut ListShadowSession<'_, 's, usize, u32, i64>,
) {
    for change in update.shadows {
        let fields = table.entry(change.value).or_default();
        assert_eq!(fields.get(change.after.property()), change.before);
        fields.apply(&change.after);
        session
            .after_variable_changed(change.variable, &change.value)
            .unwrap();
    }
    for value in update.notifications {
        session.mark_dirty(&value).unwrap();
    }
}
#[test]
fn native_move_publications_drive_shadow_suppliers_and_undo_materialized_values() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let target = s.entity::<u32>().unwrap().variable("arrival").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    let p = ListProblem::new(
        vec![1u32, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned(true),
        ],
    )
    .unwrap();
    let mut assignment = p.initial_assignment();
    let bindings = [(10usize, 0), (20usize, 1)];
    registry
        .synchronize_lists(list, &assignment, &bindings)
        .unwrap();
    let state = registry.list::<usize, u32>(list).unwrap();
    let mut supply = ListStateSupply::from_problem(state.clone(), &p, &bindings).unwrap();
    bind(&s, &mut supply);
    let calls = Cell::new(0);
    let mut session =
        ListShadowSession::new_deferred(state, target, p.values().iter().copied(), |c| {
            calls.set(calls.get() + 1);
            c.location
                .map(|_| c.previous_shadow.copied().unwrap_or(0i64) + i64::from(*c.value))
        })
        .unwrap();
    let mut table = BTreeMap::new();
    consume(supply.refresh_all().unwrap(), &mut table, &mut session);
    session.update_changed().unwrap();
    assert_eq!(calls.get(), 4);
    assert!(supply.is_pinned(&1));
    assert!(supply.is_pinned(&4));
    assert!(!supply.is_pinned(&2));
    let before = table.clone();
    let saved = session.checkpoint();
    let edit = p
        .apply(
            &mut assignment,
            &ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 2,
                },
                destination: ListPosition {
                    entity: 0,
                    index: 1,
                },
            },
        )
        .unwrap();
    registry.synchronize_lists(list, &edit, &bindings).unwrap();
    let update = supply.refresh_range(&10, 1, 3).unwrap();
    assert!(update.notifications.is_empty());
    consume(update, &mut table, &mut session);
    session.update_changed().unwrap();
    assert_eq!(table[&1].next, Some(3));
    assert_eq!(table[&3].index, Some(1));
    assert_eq!(table[&2].previous, Some(3));
    assert_eq!(session.values().unwrap()[&2].value, Some(6));
    assert_eq!(supply.unassigned_count(), 0);
    drop(edit);
    registry
        .synchronize_lists(list, &assignment, &bindings)
        .unwrap();
    consume(
        supply.refresh_range(&10, 1, 3).unwrap(),
        &mut table,
        &mut session,
    );
    let count = calls.get();
    session.restore(&saved).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(table, before);
    assert_eq!(session.values().unwrap()[&3].value, Some(6));
    assert!(
        p.apply(
            &mut assignment,
            &ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 0
                },
                destination: ListPosition {
                    entity: 0,
                    index: 1
                }
            }
        )
        .is_err()
    );
}
#[test]
fn unassign_before_first_refresh_clears_seeded_shadows_and_notifies_once() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(list, [(0usize, vec![1u32])]).unwrap();
    let mut supply = ListStateSupply::new(r.list::<usize, u32>(list).unwrap(), [1u32, 2]).unwrap();
    let e = s.entity::<u32>().unwrap();
    supply
        .externalize(ListShadowProperty::Owner, e.variable("owner").unwrap())
        .unwrap();
    supply
        .externalize(ListShadowProperty::Index, e.variable("index").unwrap())
        .unwrap();
    supply
        .seed_shadows([(
            1,
            ListBuiltinValues {
                owner: Some(0),
                index: Some(0),
                previous: None,
                next: None,
            },
        )])
        .unwrap();
    let update = supply.unassign(&mut r, &1).unwrap();
    assert_eq!(update.notifications, [1]);
    assert_eq!(update.shadows.len(), 2);
    assert_eq!(
        supply.shadow_values(&1).unwrap(),
        &ListBuiltinValues::default()
    );
    assert_eq!(supply.unassigned_count(), 2);
    assert!(supply.refresh_all().unwrap().notifications.is_empty());
}
#[test]
fn invalid_bindings_seeds_and_ranges_leave_published_state_unchanged() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let e = s.entity::<u32>().unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(list, [(0usize, vec![1u32, 2])]).unwrap();
    let state = r.list::<usize, u32>(list).unwrap();
    assert!(ListStateSupply::new(state.clone(), [1, 1]).is_err());
    assert!(ListStateSupply::new(state.clone(), [1]).is_err());
    let mut supply = ListStateSupply::new(state.clone(), [1, 2, 3]).unwrap();
    bind(&s, &mut supply);
    supply.refresh_all().unwrap();
    let before = supply.shadow_values(&1).unwrap().clone();
    assert!(
        supply
            .externalize(ListShadowProperty::Owner, e.variable("owner").unwrap())
            .is_err()
    );
    assert!(
        supply
            .seed_shadows([
                (1, ListBuiltinValues::default()),
                (9, ListBuiltinValues::default())
            ])
            .is_err()
    );
    assert!(
        supply
            .seed_shadows([
                (1, ListBuiltinValues::default()),
                (1, ListBuiltinValues::default())
            ])
            .is_err()
    );
    assert!(supply.refresh_range(&0, 2, 1).is_err());
    assert!(supply.refresh_range(&0, 0, 3).is_err());
    assert!(supply.refresh_range(&9, 0, 0).is_err());
    assert_eq!(supply.shadow_values(&1).unwrap(), &before);
    let mut foreign = VariableStateRegistry::new(&s);
    assert!(supply.unassign(&mut foreign, &1).is_err());
    assert!(supply.is_assigned(&1));
    assert!(supply.unassign(&mut r, &9).is_err());
    let model = ListProblem::new(vec![1u32, 2, 3], vec![ListEntity::new(vec![1, 2])]).unwrap();
    assert!(ListStateSupply::from_problem(state.clone(), &model, &[(1usize, 0)]).is_err());
    assert!(ListStateSupply::from_problem(state.clone(), &model, &[(0usize, 1)]).is_err());
    let mut fresh = ListStateSupply::new(state, [1, 2, 3]).unwrap();
    assert!(
        fresh
            .externalize(ListShadowProperty::Index, e.variable("arrival").unwrap())
            .is_err()
    );
    assert!(fresh.externalize(ListShadowProperty::Owner, list).is_err());
    let foreign_schema = schema();
    assert!(
        fresh
            .externalize(
                ListShadowProperty::Index,
                foreign_schema
                    .entity::<u32>()
                    .unwrap()
                    .variable("index")
                    .unwrap()
            )
            .is_err()
    );
    r.update_lists(list, [(0usize, vec![9u32])]).unwrap();
    assert!(supply.refresh_all().is_err());
    assert_eq!(supply.shadow_values(&1).unwrap(), &before);
}

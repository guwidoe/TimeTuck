//! Original explicit membership reconciliation over native shared list state.
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Route;
type Rows = Vec<(usize, Vec<u32>)>;
#[derive(Debug, Eq, PartialEq)]
struct Shadow(i64); // Membership edits preserve values without Clone.
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<u32>(
            "visits", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![PlanningVariableDefinition::shadow::<Shadow>(
            "arrival",
        )]),
    ])
    .unwrap()
}
fn supply(c: ListShadowContext<'_, usize, u32, Shadow>) -> Option<Shadow> {
    c.location.map(|p| {
        Shadow(
            i64::from(*c.value)
                + p.index as i64
                + c.previous_shadow.or(c.next_shadow).map_or(0, |h| h.0),
        )
    })
}
fn values(session: &ListShadowSession<'_, '_, usize, u32, Shadow>) -> BTreeMap<u32, Option<i64>> {
    session
        .cached_values()
        .iter()
        .map(|(&k, v)| (k, v.value.as_ref().map(|s| s.0)))
        .collect()
}
#[test]
fn membership_reindexing_and_neighbor_changes_match_fresh_sessions_in_both_directions() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let schema = schema();
        let source = schema
            .entity::<Route>()
            .unwrap()
            .variable("visits")
            .unwrap();
        let target = schema.entity::<u32>().unwrap().variable("arrival").unwrap();
        let mut registry = VariableStateRegistry::new(&schema);
        registry
            .reset_lists(source, [(0usize, vec![10u32, 20]), (1, vec![30])])
            .unwrap();
        let state = registry.list::<usize, u32>(source).unwrap();
        let calls = RefCell::new(vec![]);
        let mut session = ListShadowSession::new_with_direction(
            state.clone(),
            target,
            [10, 20, 30, 40],
            direction,
            |c| {
                calls.borrow_mut().push(*c.value);
                supply(c)
            },
        )
        .unwrap();
        let steps: [(Vec<u32>, Rows); 6] = [
            (
                vec![0, 10, 20, 30, 40],
                vec![(0, vec![0, 10, 20]), (1, vec![30])],
            ),
            (vec![0, 10, 25, 30], vec![(0, vec![0, 10]), (1, vec![30])]),
            (vec![10, 25, 30, 99], vec![(0, vec![10]), (1, vec![25, 30])]),
            (vec![], vec![]),
            (vec![5, 10], vec![(2, vec![5])]),
            (vec![0, 5, 10], vec![(2, vec![10, 5])]),
        ];
        for (universe, rows) in steps {
            let before = values(&session);
            let oldkeys = before.keys().copied().collect::<BTreeSet<_>>();
            let nextkeys = universe.iter().copied().collect::<BTreeSet<_>>();
            let saved = session.checkpoint();
            let oldrows = state.snapshot();
            registry.update_lists(source, rows).unwrap();
            calls.borrow_mut().clear();
            let report = session.reconcile_values(universe.clone()).unwrap();
            assert!(session.is_current());
            let fresh = ListShadowSession::new_with_direction(
                state.clone(),
                target,
                universe,
                direction,
                supply,
            )
            .unwrap();
            assert_eq!(values(&session), values(&fresh));
            assert_eq!(
                report.added,
                nextkeys.difference(&oldkeys).copied().collect::<Vec<_>>()
            );
            assert_eq!(
                report.removed,
                oldkeys.difference(&nextkeys).copied().collect::<Vec<_>>()
            );
            let expected = values(&session)
                .into_iter()
                .filter(|(k, v)| before.get(k).copied().flatten() != *v)
                .map(|(k, _)| k)
                .collect::<Vec<_>>();
            assert_eq!(report.changed, expected);
            // Independent list [30] stays untouched when a lower sorted key is inserted.
            if report.added == [0] && report.removed.is_empty() && nextkeys.contains(&40) {
                assert!(!calls.borrow().contains(&30));
            }
            let newrows = state.snapshot();
            let newstate = session.checkpoint();
            let count = calls.borrow().len();
            registry.update_lists(source, oldrows).unwrap();
            session.restore(&saved).unwrap();
            assert_eq!(values(&session), before);
            assert_eq!(calls.borrow().len(), count);
            registry.update_lists(source, newrows).unwrap();
            session.restore(&newstate).unwrap();
            assert_eq!(values(&session), values(&fresh));
            assert_eq!(calls.borrow().len(), count);
        }
    }
}
#[test]
fn unassigned_none_membership_is_reported_and_reintroduction_initializes_again() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("arrival").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_lists(source, [(0usize, vec![10u32])])
        .unwrap();
    let calls = RefCell::new(vec![]);
    let mut session = ListShadowSession::new(
        registry.list::<usize, u32>(source).unwrap(),
        target,
        [10],
        |c| {
            calls.borrow_mut().push(*c.value);
            supply(c)
        },
    )
    .unwrap();
    calls.borrow_mut().clear();
    let add = session.reconcile_values([0, 10]).unwrap();
    assert_eq!(
        add,
        ListShadowMembershipUpdate {
            added: vec![0],
            removed: vec![],
            changed: vec![]
        }
    );
    assert_eq!(*calls.borrow(), vec![0]);
    assert_eq!(values(&session)[&0], None);
    calls.borrow_mut().clear();
    session.mark_dirty(&0).unwrap();
    let remove = session.reconcile_values([10]).unwrap();
    assert_eq!(
        remove,
        ListShadowMembershipUpdate {
            added: vec![],
            removed: vec![0],
            changed: vec![]
        }
    );
    assert!(calls.borrow().is_empty());
    assert_eq!(session.pending_values().len(), 0);
    assert!(matches!(
        session.mark_dirty(&0),
        Err(ListShadowError::UnknownValue(0))
    ));
    let add = session.reconcile_values([10, 0]).unwrap();
    assert_eq!(add.added, [0]);
    assert_eq!(*calls.borrow(), vec![0]);
    calls.borrow_mut().clear();
    assert_eq!(
        session.reconcile_values([0, 10]).unwrap(),
        ListShadowMembershipUpdate {
            added: vec![],
            removed: vec![],
            changed: vec![]
        }
    );
    assert!(calls.borrow().is_empty());
}
#[test]
fn validation_and_supplier_panics_preserve_universe_cache_and_pending_notifications() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("arrival").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_lists(source, [(0usize, vec![10u32, 20])])
        .unwrap();
    let state = registry.list::<usize, u32>(source).unwrap();
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let mut session = ListShadowSession::new(state.clone(), target, [10, 20, 99], |c| {
        calls.set(calls.get() + 1);
        assert!(!(fail.get() && *c.value == 0), "new value supplier");
        supply(c)
    })
    .unwrap();
    let before = values(&session);
    session.mark_dirty(&10).unwrap();
    session.mark_dirty(&99).unwrap();
    let saved = session.checkpoint();
    let count = calls.get();
    assert!(matches!(
        session.reconcile_values([10, 10, 20]),
        Err(ListShadowError::Dependency(DependencyError::DuplicateNode(
            10
        )))
    ));
    assert!(matches!(
        session.reconcile_values([10]),
        Err(ListShadowError::UnknownValue(20))
    ));
    assert_eq!(calls.get(), count);
    assert_eq!(values(&session), before);
    assert_eq!(
        session.pending_values().copied().collect::<Vec<_>>(),
        [10, 99]
    );
    registry
        .update_lists(source, [(0usize, vec![0u32, 10, 20])])
        .unwrap();
    assert!(matches!(
        session.update_changed(),
        Err(ListShadowError::UnknownValue(0))
    ));
    assert_eq!(calls.get(), count);
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.reconcile_values([0, 10, 20]))).is_err());
    assert_eq!(values(&session), before);
    assert_eq!(
        session.pending_values().copied().collect::<Vec<_>>(),
        [10, 99]
    );
    assert!(!session.is_current());
    assert!(matches!(
        session.restore(&saved),
        Err(ListShadowError::SnapshotMismatch)
    ));
    registry
        .update_lists(source, [(0usize, vec![10u32, 20])])
        .unwrap();
    let count = calls.get();
    session.restore(&saved).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(values(&session), before);
    fail.set(false);
    registry
        .update_lists(source, [(0usize, vec![0u32, 10, 20])])
        .unwrap();
    let report = session.reconcile_values([0, 10, 20]).unwrap();
    assert_eq!(report.added, [0]);
    assert_eq!(report.removed, [99]);
    assert_eq!(session.pending_values().len(), 0);
    let fresh = ListShadowSession::new(state, target, [0, 10, 20], supply).unwrap();
    assert_eq!(values(&session), values(&fresh));
}
#[test]
fn adding_unrelated_values_preserves_all_survivor_supplier_work() {
    for n in [1u32, 512] {
        let schema = schema();
        let source = schema
            .entity::<Route>()
            .unwrap()
            .variable("visits")
            .unwrap();
        let target = schema.entity::<u32>().unwrap().variable("arrival").unwrap();
        let mut registry = VariableStateRegistry::new(&schema);
        registry
            .reset_lists(source, [(0usize, (1..=n).collect::<Vec<_>>())])
            .unwrap();
        let calls = Cell::new(0);
        let mut session = ListShadowSession::new(
            registry.list::<usize, u32>(source).unwrap(),
            target,
            1..=n,
            |c| {
                calls.set(calls.get() + 1);
                supply(c)
            },
        )
        .unwrap();
        assert_eq!(calls.get(), n as usize);
        let before = values(&session);
        let report = session.reconcile_values(0..=n).unwrap();
        assert_eq!(report.added, [0]);
        assert!(report.changed.is_empty());
        assert_eq!(calls.get(), n as usize + 1);
        let after = values(&session);
        for (k, v) in before {
            assert_eq!(after[&k], v);
        }
        session.reconcile_values(1..=n).unwrap();
        assert_eq!(calls.get(), n as usize + 1);
        session.reconcile_values((1..=n).rev()).unwrap();
        assert_eq!(calls.get(), n as usize + 1);
    }
}
#[test]
fn deferred_initialization_can_reconcile_before_any_supplier_has_run() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("arrival").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_lists(source, [(0usize, vec![10u32])])
        .unwrap();
    let state = registry.list::<usize, u32>(source).unwrap();
    let calls = Cell::new(0);
    let mut session = ListShadowSession::new_deferred(state.clone(), target, [10, 99], |c| {
        calls.set(calls.get() + 1);
        supply(c)
    })
    .unwrap();
    let saved = session.checkpoint();
    assert_eq!(calls.get(), 0);
    registry
        .update_lists(source, [(0usize, vec![0u32, 10])])
        .unwrap();
    let report = session.reconcile_values([0, 10]).unwrap();
    assert_eq!(report.added, [0]);
    assert_eq!(report.removed, [99]);
    assert_eq!(calls.get(), 2);
    assert!(session.is_current());
    let fresh = ListShadowSession::new(state, target, [0, 10], supply).unwrap();
    assert_eq!(values(&session), values(&fresh));
    registry
        .update_lists(source, [(0usize, vec![10u32])])
        .unwrap();
    let count = calls.get();
    session.restore(&saved).unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(
        session.pending_values().copied().collect::<Vec<_>>(),
        [10, 99]
    );
    assert_eq!(values(&session), BTreeMap::from([(10, None), (99, None)]));
}

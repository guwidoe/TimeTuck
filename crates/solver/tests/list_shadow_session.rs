//! Original metadata-bound shadow execution and transaction regressions.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Route;
#[derive(Debug, Eq, PartialEq)]
struct Shadow(i64);
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<u32>(
            "visits", true,
        )]),
        PlanningEntityDefinition::of::<u32>(vec![
            PlanningVariableDefinition::shadow::<u32>("previous"),
            PlanningVariableDefinition::shadow::<usize>("owner"),
            PlanningVariableDefinition::shadow::<Shadow>("arrival"),
            PlanningVariableDefinition::shadow::<i64>("count"),
            PlanningVariableDefinition::basic::<i32>("extra", false),
        ]),
    ])
    .unwrap()
}
#[test]
fn actual_moves_match_fresh_cascades_and_restore_without_supplier_calls() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let target = s.entity::<u32>().unwrap().variable("count").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    let p = ListProblem::new(
        vec![1u32, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut assignment = p.initial_assignment();
    let bindings = [(10usize, 0), (20usize, 1)];
    registry
        .synchronize_lists(list, &assignment, &bindings)
        .unwrap();
    let state = registry.list::<usize, u32>(list).unwrap();
    let calls = Cell::new(0usize);
    let mut session = ListShadowSession::new(state.clone(), target, [1u32, 2, 3], |c| {
        calls.set(calls.get() + 1);
        c.location
            .map(|_| c.previous_shadow.copied().unwrap_or(0) + i64::from(*c.value))
    })
    .unwrap();
    for source in [
        ListPosition {
            entity: 0,
            index: 0,
        },
        ListPosition {
            entity: 0,
            index: 1,
        },
        ListPosition {
            entity: 1,
            index: 0,
        },
    ] {
        let saved = session.checkpoint();
        let before = state.snapshot();
        let to = if source.entity == 0 {
            ListPosition {
                entity: 1,
                index: 1,
            }
        } else {
            ListPosition {
                entity: 0,
                index: 1,
            }
        };
        let edit = p
            .apply(
                &mut assignment,
                &ListMove::Change {
                    source,
                    destination: to,
                },
            )
            .unwrap();
        registry.synchronize_lists(list, &edit, &bindings).unwrap();
        assert!(session.values().is_err());
        session.update_changed().unwrap();
        let fresh = ListCascadeCache::new(&p, &edit, |c| {
            c.position
                .map(|_| c.previous_shadow.copied().unwrap_or(0) + i64::from(*c.value))
        })
        .unwrap();
        for value in p.values() {
            assert_eq!(
                session.values().unwrap()[value].value,
                fresh.shadows()[value]
            );
        }
        assert_eq!(session.source(), list);
        assert_eq!(session.target(), target);
        assert!(matches!(
            session.restore(&saved),
            Err(ListShadowError::SnapshotMismatch)
        ));
        drop(edit);
        registry
            .synchronize_lists(list, &assignment, &bindings)
            .unwrap();
        let count = calls.get();
        session.restore(&saved).unwrap();
        assert_eq!(calls.get(), count);
        assert_eq!(state.snapshot(), before);
        assert!(session.is_current());
    }
}
#[test]
fn deferred_notifications_use_latest_snapshot_and_skip_equal_value_propagation() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let e = s.entity::<u32>().unwrap();
    let target = e.variable("count").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    registry
        .reset_lists(list, [(0usize, vec![1u32, 2, 3])])
        .unwrap();
    let calls = RefCell::new(Vec::new());
    let mut session = ListShadowSession::new_deferred(
        registry.list::<usize, u32>(list).unwrap(),
        target,
        [1u32, 2, 3, 4],
        |c| {
            calls.borrow_mut().push(*c.value);
            c.location
                .map(|_| c.previous_shadow.copied().unwrap_or(0i64) + 1)
        },
    )
    .unwrap();
    assert_eq!(session.pending_values().count(), 4);
    assert!(calls.borrow().is_empty());
    assert!(session.values().is_err());
    session.update_changed().unwrap();
    assert_eq!(calls.borrow().len(), 4);
    calls.borrow_mut().clear();
    session.mark_dirty(&1).unwrap();
    session.mark_dirty(&1).unwrap();
    assert_eq!(session.pending_values().count(), 1);
    assert!(session.update_changed().unwrap().is_empty());
    assert_eq!(*calls.borrow(), [1]);
    calls.borrow_mut().clear();
    registry
        .update_lists(list, [(0usize, vec![2u32, 1, 3])])
        .unwrap();
    session
        .after_variable_changed(e.variable("previous").unwrap(), &1)
        .unwrap();
    registry
        .update_lists(list, [(5usize, vec![4u32, 3, 2, 1])])
        .unwrap();
    session
        .after_variable_changed(e.variable("owner").unwrap(), &1)
        .unwrap();
    assert!(calls.borrow().is_empty());
    session.update_changed().unwrap();
    assert_eq!(session.values().unwrap()[&1].value, Some(4));
    assert_eq!(session.values().unwrap()[&4].value, Some(1));
    let mut seen = calls.borrow().clone();
    seen.sort();
    assert_eq!(seen, [1, 2, 3, 4]);
    calls.borrow_mut().clear();
    assert!(session.update_changed().unwrap().is_empty());
    assert!(calls.borrow().is_empty());
    registry.update_lists::<usize, u32>(list, []).unwrap();
    session.update_changed().unwrap();
    assert!(
        session
            .values()
            .unwrap()
            .values()
            .all(|v| v.value.is_none())
    );
}
#[test]
fn supplier_panic_preserves_non_clone_results_pending_work_and_checkpoints() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let target = s.entity::<u32>().unwrap().variable("arrival").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(list, [(0usize, vec![1u32, 2, 3])]).unwrap();
    let panic_at = Cell::new(false);
    let offset = Cell::new(0i64);
    let mut session = ListShadowSession::new(
        r.list::<usize, u32>(list).unwrap(),
        target,
        [1u32, 2, 3],
        |c| {
            assert!(!panic_at.get() || *c.value != 2, "supplier panic");
            c.location.map(|_| {
                Shadow(
                    c.previous_shadow.map_or(offset.get(), |v: &Shadow| v.0) + i64::from(*c.value),
                )
            })
        },
    )
    .unwrap();
    let saved = session.checkpoint();
    session.mark_dirty(&1).unwrap();
    offset.set(10);
    panic_at.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.update_changed())).is_err());
    assert_eq!(session.pending_values().copied().collect::<Vec<_>>(), [1]);
    assert_eq!(session.cached_values()[&1].value, Some(Shadow(1)));
    assert_eq!(session.cached_values()[&3].value, Some(Shadow(6)));
    assert!(session.values().is_err());
    panic_at.set(false);
    session.update_changed().unwrap();
    assert_eq!(session.values().unwrap()[&3].value, Some(Shadow(16)));
    offset.set(0);
    session.restore(&saved).unwrap();
    assert_eq!(session.values().unwrap()[&3].value, Some(Shadow(6)));
}
#[test]
fn metadata_universe_and_foreign_snapshot_errors_do_not_publish_results() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let e = s.entity::<u32>().unwrap();
    let target = e.variable("count").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(list, [(0usize, vec![1u32])]).unwrap();
    let state = r.list::<usize, u32>(list).unwrap();
    let calls = Cell::new(0);
    assert!(
        ListShadowSession::new(state.clone(), target, [1u32, 1], |_| {
            calls.set(calls.get() + 1);
            Some(0i64)
        })
        .is_err()
    );
    assert!(
        ListShadowSession::new(state.clone(), target, [2u32], |_| {
            calls.set(calls.get() + 1);
            Some(0i64)
        })
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(ListShadowSession::new(state.clone(), target, [1u32], |_| Some(0i32)).is_err());
    assert!(
        ListShadowSession::new(state.clone(), e.variable("extra").unwrap(), [1u32], |_| {
            Some(0i32)
        })
        .is_err()
    );
    let other = schema();
    assert!(
        ListShadowSession::new(
            state.clone(),
            other.entity::<u32>().unwrap().variable("count").unwrap(),
            [1u32],
            |_| Some(0i64)
        )
        .is_err()
    );
    let mut session =
        ListShadowSession::new(state.clone(), target, [1u32, 2], |_| Some(0i64)).unwrap();
    let other = ListShadowSession::new(state, target, [1u32, 2], |_| Some(0i64)).unwrap();
    assert!(matches!(
        session.restore(&other.checkpoint()),
        Err(ListShadowError::ForeignCheckpoint)
    ));
    assert!(session.after_variable_changed(list, &1).is_err());
    assert!(session.mark_dirty(&9).is_err());
    assert!(session.is_current());
    r.update_lists(list, [(0usize, vec![9u32])]).unwrap();
    assert!(matches!(
        session.update_changed(),
        Err(ListShadowError::UnknownValue(9))
    ));
    assert_eq!(session.cached_values()[&1].value, Some(0));
    assert!(session.values().is_err());
    r.update_lists(list, [(0usize, vec![1u32])]).unwrap();
    assert!(session.is_current());
    let snapshot: BTreeMap<_, _> = r.list::<usize, u32>(list).unwrap().snapshot();
    assert_eq!(snapshot[&0], vec![1]);
}

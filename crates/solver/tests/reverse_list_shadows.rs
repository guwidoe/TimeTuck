//! Original bidirectional cascade regressions; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Schedule;
struct Route;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Schedule>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<i64>(
            "visits", true,
        )]),
        PlanningEntityDefinition::of::<i64>(vec![PlanningVariableDefinition::shadow::<i64>(
            "total",
        )]),
    ])
    .unwrap()
}
fn reference(
    a: &ListAssignment<i64>,
    direction: ListCascadeDirection,
) -> BTreeMap<i64, Option<i64>> {
    let mut result: BTreeMap<_, _> = (1..=7).map(|v| (v, None)).collect();
    for list in a.lists() {
        for (i, &v) in list.iter().enumerate() {
            let segment = match direction {
                ListCascadeDirection::Previous => &list[..=i],
                ListCascadeDirection::Next => &list[i..],
            };
            result.insert(v, Some(segment.iter().sum()));
        }
    }
    result
}
#[test]
fn reordered_partial_lists_match_independent_sums_and_restore_both_caches() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let p = ListProblem::new(
            (1..=7).collect(),
            vec![ListEntity::new(vec![3, 1, 5]), ListEntity::new(vec![4, 2])],
        )
        .unwrap()
        .allowing_unassigned();
        let s = schema();
        let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
        let target = s.entity::<i64>().unwrap().variable("total").unwrap();
        let mut registry = VariableStateRegistry::new(&s);
        let mut a = p.initial_assignment();
        let bindings = [(0usize, 0), (1usize, 1)];
        registry.synchronize_lists(list, &a, &bindings).unwrap();
        let mut cache = ListCascadeCache::new_with_direction(&p, &a, direction, |c| {
            let parent = match direction {
                ListCascadeDirection::Previous => {
                    assert!(c.next_shadow.is_none());
                    c.previous_shadow
                }
                ListCascadeDirection::Next => {
                    assert!(c.previous_shadow.is_none());
                    c.next_shadow
                }
            };
            c.position.map(|_| parent.copied().unwrap_or(0) + *c.value)
        })
        .unwrap();
        let calls = Cell::new(0);
        let mut session = ListShadowSession::new_with_direction(
            registry.list::<usize, i64>(list).unwrap(),
            target,
            1..=7,
            direction,
            |c| {
                calls.set(calls.get() + 1);
                let parent = match direction {
                    ListCascadeDirection::Previous => {
                        assert!(c.next_shadow.is_none());
                        c.previous_shadow
                    }
                    ListCascadeDirection::Next => {
                        assert!(c.previous_shadow.is_none());
                        c.next_shadow
                    }
                };
                c.location.map(|_| parent.copied().unwrap_or(0) + *c.value)
            },
        )
        .unwrap();
        assert_eq!(cache.direction(), direction);
        assert_eq!(session.direction(), direction);
        let mut seed = 871u64;
        for round in 0..160 {
            let saved_cache = cache.clone();
            let saved_session = session.checkpoint();
            let old = reference(&a, direction);
            let mut order: Vec<_> = (1..=7).collect();
            for i in (1..order.len()).rev() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                order.swap(i, (seed >> 32) as usize % (i + 1));
            }
            let mut rows = vec![vec![], vec![]];
            for value in order {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let owner = (seed >> 32) as usize % 3;
                if owner < 2 {
                    rows[owner].push(value);
                }
            }
            let next = ListAssignment::new(rows);
            let expected = reference(&next, direction);
            let changed = cache
                .update(&next, &[], |c| {
                    c.position.map(|_| {
                        let parent = match direction {
                            ListCascadeDirection::Previous => c.previous_shadow,
                            ListCascadeDirection::Next => c.next_shadow,
                        };
                        parent.copied().unwrap_or(0) + *c.value
                    })
                })
                .unwrap();
            registry.synchronize_lists(list, &next, &bindings).unwrap();
            let session_changed = session.update_changed().unwrap();
            let expected_changed: Vec<_> = expected
                .iter()
                .filter(|(v, total)| old[v] != **total)
                .map(|(&v, _)| v)
                .collect();
            assert_eq!(changed, expected_changed);
            assert_eq!(session_changed, expected_changed);
            for (&v, &total) in &expected {
                assert_eq!(cache.shadows()[&v], total);
                assert_eq!(session.values().unwrap()[&v].value, total);
                assert!(!session.values().unwrap()[&v].inconsistent);
            }
            if round % 3 == 0 {
                a = next;
            } else {
                cache = saved_cache;
                registry.synchronize_lists(list, &a, &bindings).unwrap();
                let before_calls = calls.get();
                session.restore(&saved_session).unwrap();
                assert_eq!(calls.get(), before_calls);
                for (&v, &total) in &old {
                    assert_eq!(cache.shadows()[&v], total);
                    assert_eq!(session.values().unwrap()[&v].value, total);
                }
            }
        }
    }
}
#[test]
fn reverse_deferred_work_preserves_seeds_stops_at_equal_values_and_retries_panics() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2, 3, 4])]).unwrap();
    let a = p.initial_assignment();
    let mut cache =
        ListCascadeCache::<_, i64>::new_deferred_with_direction(&p, &a, ListCascadeDirection::Next)
            .unwrap();
    let calls = RefCell::new(vec![]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| cache.flush_pending(|c| {
            calls.borrow_mut().push(*c.value);
            assert_ne!(*c.value, 2);
            Some(c.next_shadow.copied().unwrap_or(0) + *c.value)
        })))
        .is_err()
    );
    assert_eq!(*calls.borrow(), [4, 3, 2]);
    assert_eq!(cache.pending_updates().count(), 4);
    assert!(cache.shadows().values().all(Option::is_none));
    calls.borrow_mut().clear();
    cache
        .flush_pending(|c| {
            calls.borrow_mut().push(*c.value);
            Some(c.next_shadow.copied().unwrap_or(0) + *c.value)
        })
        .unwrap();
    assert_eq!(*calls.borrow(), [4, 3, 2, 1]);
    assert_eq!(cache.shadows()[&1], Some(10));
    // Unchanged tail stops propagation; an independently dirty head still runs.
    cache.queue_update(&a, &[4, 1]).unwrap();
    calls.borrow_mut().clear();
    assert!(
        cache
            .flush_pending(|c| {
                calls.borrow_mut().push(*c.value);
                Some(c.next_shadow.copied().unwrap_or(0) + *c.value)
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(*calls.borrow(), [4, 1]);
    let mut seeded =
        ListCascadeCache::from_values_with_direction(&p, &a, ListCascadeDirection::Next, [(4, 40)])
            .unwrap();
    seeded
        .update(&a, &[3], |c| {
            Some(c.next_shadow.copied().unwrap_or(0) + *c.value)
        })
        .unwrap();
    assert_eq!(seeded.shadows()[&1], Some(46));
    assert_eq!(seeded.shadows()[&4], Some(40));
}
#[test]
fn reverse_session_retains_notifications_after_supplier_failure() {
    let s = schema();
    let list = s.entity::<Route>().unwrap().variable("visits").unwrap();
    let target = s.entity::<i64>().unwrap().variable("total").unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    registry
        .reset_lists(list, [(0usize, vec![1i64, 2, 3])])
        .unwrap();
    let calls = RefCell::new(vec![]);
    let fail = Cell::new(false);
    let tail = Cell::new(3);
    let mut session = ListShadowSession::new_deferred_with_direction(
        registry.list::<usize, i64>(list).unwrap(),
        target,
        1..=3,
        ListCascadeDirection::Next,
        |c| {
            calls.borrow_mut().push(*c.value);
            assert!(!fail.get() || *c.value != 2);
            c.location.map(|_| {
                c.next_shadow.copied().unwrap_or(0)
                    + if *c.value == 3 { tail.get() } else { *c.value }
            })
        },
    )
    .unwrap();
    session.update_changed().unwrap();
    assert_eq!(*calls.borrow(), [3, 2, 1]);
    let old = session.values().unwrap().clone();
    tail.set(30);
    session.after_variable_changed(target, &3).unwrap();
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.update_changed())).is_err());
    assert_eq!(session.cached_values(), &old);
    assert_eq!(session.pending_values().copied().collect::<Vec<_>>(), [3]);
    fail.set(false);
    calls.borrow_mut().clear();
    assert_eq!(session.update_changed().unwrap(), [1, 2, 3]);
    assert_eq!(*calls.borrow(), [3, 2, 1]);
    assert_eq!(session.values().unwrap()[&1].value, Some(33));
    calls.borrow_mut().clear();
    session.mark_dirty(&3).unwrap();
    assert!(session.update_changed().unwrap().is_empty());
    assert_eq!(*calls.borrow(), [3]);
}

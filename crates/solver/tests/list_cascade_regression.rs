use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn position(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2, 3]),
            ListEntity::new(vec![4]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}
fn arrival(c: ListCascadeContext<'_, i32, i64>) -> Option<i64> {
    c.position
        .map(|p| c.previous_shadow.copied().unwrap_or(10 * p.entity as i64) + i64::from(*c.value))
}
fn full(a: &ListAssignment<i32>) -> BTreeMap<i32, Option<i64>> {
    let mut expected = (1..=4).map(|v| (v, None)).collect::<BTreeMap<_, _>>();
    for (entity, list) in a.lists().iter().enumerate() {
        let mut sum = 10 * entity as i64;
        for v in list {
            sum += i64::from(*v);
            expected.insert(*v, Some(sum));
        }
    }
    expected
}
fn assert_fresh(cache: &ListCascadeCache<i32, i64>) {
    assert_eq!(
        cache
            .shadows()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<BTreeMap<_, _>>(),
        full(cache.assignment())
    );
}
#[test]
fn transitions_match_independent_folds_for_every_small_partial_assignment() {
    fn enumerate(remaining: Vec<i32>, prefix: Vec<i32>, out: &mut Vec<Vec<i32>>) {
        out.push(prefix.clone());
        for i in 0..remaining.len() {
            let mut next = remaining.clone();
            let v = next.remove(i);
            let mut p = prefix.clone();
            p.push(v);
            enumerate(next, p, out);
        }
    }
    let p = problem();
    let mut cache = ListCascadeCache::new(&p, &p.initial_assignment(), arrival).unwrap();
    let mut orders = vec![];
    enumerate(vec![1, 2, 3, 4], vec![], &mut orders);
    for order in orders {
        for first in 0..=order.len() {
            for second in first..=order.len() {
                let a = ListAssignment::new(vec![
                    order[..first].to_vec(),
                    order[first..second].to_vec(),
                    order[second..].to_vec(),
                ]);
                let before = cache.shadows().clone();
                let expected = full(&a);
                let mut calls = BTreeMap::new();
                let changed = cache
                    .update(&a, &[], |c| {
                        *calls.entry(*c.value).or_insert(0) += 1;
                        let element = a.element(c.value);
                        assert_eq!(c.position, element.map(|e| e.position));
                        assert_eq!(c.previous, element.and_then(|e| e.previous));
                        assert_eq!(c.next, element.and_then(|e| e.next));
                        arrival(c)
                    })
                    .unwrap();
                assert!(calls.values().all(|&n| n == 1));
                assert_eq!(
                    changed,
                    expected
                        .iter()
                        .filter_map(|(v, s)| (before[v] != *s).then_some(*v))
                        .collect::<Vec<_>>()
                );
                assert_fresh(&cache);
            }
        }
    }
}
#[test]
fn equal_values_stop_propagation_but_later_dirty_roots_still_run() {
    let p = problem();
    let a = p.initial_assignment();
    let mut cache = ListCascadeCache::new(&p, &a, |c| Some(*c.value)).unwrap();
    let mut calls = vec![];
    assert!(
        cache
            .update(&a, &[1, 3, 1], |c| {
                calls.push(*c.value);
                Some(*c.value)
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(calls, [1, 3]);
    cache
        .update(&a, &[], |_| {
            panic!("unchanged state must not invoke suppliers")
        })
        .unwrap();
    // A changed external fact propagates through the affected list only.
    let mut cache = ListCascadeCache::new(&p, &a, arrival).unwrap();
    let mut calls = vec![];
    assert_eq!(
        cache
            .update(&a, &[2], |c| {
                calls.push(*c.value);
                let extra = i64::from(*c.value == 2) * 100;
                arrival(c).map(|v| v + extra)
            })
            .unwrap(),
        [2, 3]
    );
    assert_eq!(calls, [2, 3]);
    assert_eq!(cache.shadows()[&3], Some(106));
    assert_eq!(cache.shadows()[&4], Some(14));
}
#[test]
fn native_move_trials_restore_or_commit_assignment_and_all_shadows() {
    let p = problem();
    let mut cache = ListCascadeCache::new(&p, &p.initial_assignment(), arrival).unwrap();
    let saved = cache.clone();
    let swap = ListMove::Swap {
        left: position(0, 0),
        right: position(1, 0),
    };
    {
        let trial = cache.apply(&swap, &[], arrival).unwrap();
        assert_fresh(&trial);
        assert!(!trial.changed().is_empty());
        assert_eq!(
            trial.assignment().lists(),
            &[vec![4, 2, 3], vec![1], vec![]]
        );
    }
    assert_eq!(cache.assignment(), saved.assignment());
    assert_fresh(&cache);
    cache.apply(&swap, &[], arrival).unwrap().commit();
    assert_fresh(&cache);
    assert_eq!(saved.assignment(), &p.initial_assignment());
    let remove = ListMove::Unassign {
        entity: 0,
        start: 1,
        end: 3,
    };
    cache.apply(&remove, &[], arrival).unwrap().commit();
    assert_eq!(cache.shadows()[&2], None);
    assert_eq!(cache.shadows()[&3], None);
    assert_fresh(&cache);
    let add = ListMove::Assign {
        value: 3,
        destination: position(2, 0),
    };
    cache.apply(&add, &[], arrival).unwrap().commit();
    assert_fresh(&cache);
    assert_eq!(cache.shadows()[&3], Some(23));
    let composite = ListMove::Composite(vec![
        ListMove::Change {
            source: position(2, 0),
            destination: position(0, 1),
        },
        ListMove::Assign {
            value: 2,
            destination: position(1, 0),
        },
    ]);
    cache.apply(&composite, &[], arrival).unwrap().commit();
    assert_fresh(&cache);
}
#[test]
fn invalid_metadata_and_supplier_unwind_preserve_state() {
    let p = problem();
    let a = p.initial_assignment();
    let mut cache = ListCascadeCache::new(&p, &a, arrival).unwrap();
    let before = cache.clone();
    assert!(matches!(
        cache.update(&a, &[99], |_| panic!("invalid dirty ID")),
        Err(ListCascadeError::UnknownValue(99))
    ));
    assert!(
        cache
            .update(
                &ListAssignment::new(vec![vec![1, 1], vec![], vec![]]),
                &[],
                |_| panic!("duplicate assignment")
            )
            .is_err()
    );
    assert!(
        cache
            .apply(
                &ListMove::Assign {
                    value: 99,
                    destination: position(0, 0)
                },
                &[],
                |_| panic!("invalid move")
            )
            .is_err()
    );
    let failed = catch_unwind(AssertUnwindSafe(|| {
        cache
            .update(&a, &[1], |c| {
                if *c.value == 2 {
                    panic!("callback failure");
                }
                Some(100)
            })
            .unwrap();
    }));
    assert!(failed.is_err());
    assert_eq!(cache.assignment(), before.assignment());
    assert_fresh(&cache);
    let failed = catch_unwind(AssertUnwindSafe(|| {
        let _trial = cache
            .apply(
                &ListMove::Swap {
                    left: position(0, 0),
                    right: position(1, 0),
                },
                &[],
                |c| {
                    if *c.value == 2 {
                        panic!("callback failure");
                    }
                    Some(100)
                },
            )
            .unwrap();
    }));
    assert!(failed.is_err());
    assert_eq!(cache.assignment(), before.assignment());
    assert_fresh(&cache);
    assert!(matches!(
        ListCascadeCache::from_values(&p, &a, [(1, 1), (1, 2)]),
        Err(ListCascadeError::DuplicateSeed(1))
    ));
    assert!(matches!(
        ListCascadeCache::from_values(&p, &a, [(99, 1)]),
        Err(ListCascadeError::UnknownValue(99))
    ));
    let pinned = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]).with_pinned_prefix(1)],
    )
    .unwrap();
    let mut pinned_cache =
        ListCascadeCache::new(&pinned, &pinned.initial_assignment(), arrival).unwrap();
    assert!(
        pinned_cache
            .update(&ListAssignment::new(vec![vec![2, 1]]), &[], |_| panic!(
                "pin changed"
            ))
            .is_err()
    );
}
#[test]
fn grouped_shadows_propagate_on_any_field_change_and_share_nonclone_values() {
    #[derive(Eq, PartialEq, Debug)]
    struct Shadow {
        stable: i32,
        arrival: i64,
    }
    let p = problem();
    let a = p.initial_assignment();
    let mut cache = ListCascadeCache::new(&p, &a, |c| {
        Some(Shadow {
            stable: *c.value,
            arrival: c.previous_shadow.map_or(0, |s: &Shadow| s.arrival) + 1,
        })
    })
    .unwrap();
    let snapshot = cache.clone();
    let mut calls = vec![];
    cache
        .update(&a, &[1], |c| {
            calls.push(*c.value);
            Some(Shadow {
                stable: *c.value,
                arrival: c.previous_shadow.map_or(10, |s| s.arrival) + 1,
            })
        })
        .unwrap();
    assert_eq!(calls, [1, 2, 3]);
    assert_eq!(cache.shadows()[&3].as_ref().unwrap().arrival, 13);
    assert_eq!(snapshot.shadows()[&3].as_ref().unwrap().arrival, 3);
    assert!(std::ptr::eq(
        cache.shadows()[&4].as_ref().unwrap(),
        snapshot.shadows()[&4].as_ref().unwrap()
    ));
}
#[test]
fn exact_solver_uses_cached_arrival_times_to_choose_shorter_routes() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![3, 2, 1])]).unwrap();
    let baseline = ListCascadeCache::new(&p, &p.initial_assignment(), arrival).unwrap();
    let result = exhaustive_list_search(
        &p,
        |a| {
            let mut trial = baseline.clone();
            trial.update(a, &[], arrival).unwrap();
            SimpleScore::new(-trial.shadows().iter().map(|(_, v)| v.unwrap()).sum::<i64>())
        },
        100,
    );
    assert!(result.is_optimal());
    let best = result.best.unwrap();
    assert_eq!(best.assignment.lists(), &[vec![1, 2, 3]]);
    assert_eq!(best.score, SimpleScore::new(-10));
    assert_eq!(baseline.assignment(), &p.initial_assignment());
    assert_fresh_for_three(&baseline);
}
fn assert_fresh_for_three(cache: &ListCascadeCache<i32, i64>) {
    assert_eq!(
        (
            cache.shadows()[&3],
            cache.shadows()[&2],
            cache.shadows()[&1]
        ),
        (Some(3), Some(5), Some(6))
    );
}

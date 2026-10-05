use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2, 3]), ListEntity::new(vec![4])],
    )
    .unwrap()
    .allowing_unassigned()
}
fn arrival(c: ListCascadeContext<'_, i32, i64>) -> Option<i64> {
    c.position
        .map(|p| c.previous_shadow.copied().unwrap_or(10 * p.entity as i64) + i64::from(*c.value))
}
fn values(c: &ListCascadeCache<i32, i64>) -> BTreeMap<i32, Option<i64>> {
    c.shadows().iter().map(|(k, v)| (*k, *v)).collect()
}
fn pending(c: &ListCascadeCache<i32, i64>) -> Vec<i32> {
    c.pending_updates().copied().collect()
}
fn full(a: &ListAssignment<i32>) -> BTreeMap<i32, Option<i64>> {
    let mut out = (1..=4).map(|v| (v, None)).collect::<BTreeMap<_, _>>();
    for (entity, list) in a.lists().iter().enumerate() {
        let mut sum = 10 * entity as i64;
        for v in list {
            sum += i64::from(*v);
            out.insert(*v, Some(sum));
        }
    }
    out
}
#[test]
fn deferred_initialization_deduplicates_and_propagates_in_list_order() {
    let p = problem();
    let a = p.initial_assignment();
    let mut c = ListCascadeCache::new_deferred(&p, &a).unwrap();
    assert_eq!(pending(&c), [1, 2, 3, 4]);
    assert!(c.shadows().iter().all(|(_, v)| v.is_none()));
    c.queue_update(&a, &[3, 1, 3]).unwrap();
    let mut called = vec![];
    assert_eq!(
        c.flush_pending(|x| {
            called.push(*x.value);
            arrival(x)
        })
        .unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(called, [1, 2, 3, 4]);
    assert_eq!(values(&c), full(&a));
    assert!(pending(&c).is_empty());
    assert!(
        c.flush_pending(|_| panic!("empty queue"))
            .unwrap()
            .is_empty()
    );
    c.queue_update(&a, &[2, 2]).unwrap();
    called.clear();
    c.flush_pending(|x| {
        called.push(*x.value);
        arrival(x)
    })
    .unwrap();
    assert_eq!(called, [2], "equal result stops propagation");
}
#[test]
fn cancellation_preserves_assignment_and_imported_shadows_until_explicit_repair() {
    let p = problem();
    let a = p.initial_assignment();
    let mut c = ListCascadeCache::from_values(&p, &a, [(1, 99i64), (3, 77)]).unwrap();
    let before = values(&c);
    let next = ListAssignment::new(vec![vec![4, 3], vec![2, 1]]);
    c.queue_update(&next, &[1, 2, 3, 4]).unwrap();
    assert_eq!(values(&c), before);
    assert_eq!(c.assignment().lists(), next.lists());
    assert_eq!(c.clear_pending_updates(), 4);
    assert_eq!(c.clear_pending_updates(), 0);
    c.flush_pending(|_| panic!("cancelled calculations must stay cancelled"))
        .unwrap();
    // Installing the same state must not resurrect cancelled structural changes.
    c.update(&next, &[], |_| panic!("same metadata")).unwrap();
    assert_eq!(values(&c), before);
    c.queue_update(&next, p.values()).unwrap();
    c.flush_pending(arrival).unwrap();
    assert_eq!(values(&c), full(&next));
}
#[test]
fn staged_moves_restore_pending_work_on_drop_and_eager_trials_flush_atomically() {
    let p = problem();
    let a = p.initial_assignment();
    let mut c = ListCascadeCache::new(&p, &a, arrival).unwrap();
    c.queue_update(&a, &[2]).unwrap();
    let before = values(&c);
    let movement = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    {
        let trial = c.queue_move(&movement, &[]).unwrap();
        assert!(trial.changed().is_empty());
        assert_eq!(trial.assignment().lists(), &[vec![4, 2, 3], vec![1]]);
        assert_eq!(values(&trial), before);
        assert!(trial.pending_updates().len() > 1);
    }
    assert_eq!(c.assignment().lists(), a.lists());
    assert_eq!(pending(&c), [2]);
    c.queue_move(&movement, &[]).unwrap().commit();
    let queued = c.clone();
    let no_move = ListMove::Composite(vec![]);
    {
        let trial = c.apply(&no_move, &[], arrival).unwrap();
        assert!(trial.pending_updates().next().is_none());
        assert_eq!(values(&trial), full(trial.assignment()));
        assert!(!trial.changed().is_empty());
    }
    assert_eq!(values(&c), values(&queued));
    assert_eq!(pending(&c), pending(&queued));
    assert_eq!(c.assignment().lists(), queued.assignment().lists());
    c.apply(&no_move, &[], arrival).unwrap().commit();
    assert_eq!(values(&c), full(c.assignment()));
    assert!(pending(&c).is_empty());
    assert_eq!(values(&queued), before, "snapshots remain unchanged");
}
#[test]
fn errors_and_mid_flush_panics_retain_the_queue_and_every_shadow() {
    let p = problem();
    let a = p.initial_assignment();
    let mut c = ListCascadeCache::new_deferred(&p, &a).unwrap();
    let old_values = values(&c);
    assert!(matches!(
        c.queue_update(&a, &[8]),
        Err(ListCascadeError::UnknownValue(8))
    ));
    assert!(
        c.queue_update(&ListAssignment::new(vec![vec![1, 1], vec![]]), &[])
            .is_err()
    );
    assert!(
        c.queue_move(
            &ListMove::Unassign {
                entity: 0,
                start: 9,
                end: 10
            },
            &[]
        )
        .is_err()
    );
    assert_eq!(c.assignment().lists(), a.lists());
    assert_eq!(pending(&c), [1, 2, 3, 4]);
    let mut calls = vec![];
    assert!(
        catch_unwind(AssertUnwindSafe(|| c.flush_pending(|x| {
            calls.push(*x.value);
            assert_ne!(*x.value, 3, "injected supplier panic");
            arrival(x)
        })))
        .is_err()
    );
    assert_eq!(calls, [1, 2, 3]);
    assert_eq!(values(&c), old_values);
    assert_eq!(pending(&c), [1, 2, 3, 4]);
    assert_eq!(c.assignment().lists(), a.lists());
    c.flush_pending(arrival).unwrap();
    assert_eq!(values(&c), full(&a));
}
#[test]
fn accumulated_structural_changes_match_independent_final_folds() {
    let p = problem();
    let mut c = ListCascadeCache::new(&p, &p.initial_assignment(), arrival).unwrap();
    for batch in 0..128 {
        let before = values(&c);
        let mut final_lists = vec![];
        for step in 0..4 {
            let mut lists = vec![vec![], vec![]];
            for i in 0..4 {
                let value = ((i + batch + step) % 4 + 1) as i32;
                let destination = (value as usize + batch / 4 + step) % 3;
                if destination < 2 {
                    lists[destination].push(value);
                }
            }
            final_lists = lists;
            c.queue_update(&ListAssignment::new(final_lists.clone()), &[])
                .unwrap();
            assert_eq!(values(&c), before, "queueing must not run callbacks");
        }
        assert_eq!(c.assignment().lists(), final_lists);
        let mut calls = BTreeMap::new();
        c.flush_pending(|x| {
            *calls.entry(*x.value).or_insert(0) += 1;
            arrival(x)
        })
        .unwrap();
        assert!(calls.values().all(|&n| n == 1));
        assert_eq!(values(&c), full(c.assignment()), "batch {batch}");
        assert!(pending(&c).is_empty());
    }
}
#[test]
fn non_clone_shadows_and_cloned_queues_are_independent() {
    #[derive(Debug, PartialEq, Eq)]
    struct Shadow(i32);
    let p = problem();
    let a = p.initial_assignment();
    let mut c = ListCascadeCache::from_values(&p, &a, [(1, Shadow(42))]).unwrap();
    c.queue_update(&a, &[2, 4]).unwrap();
    let mut snapshot = c.clone();
    assert_eq!(c.clear_pending_updates(), 2);
    assert_eq!(
        snapshot.pending_updates().copied().collect::<Vec<_>>(),
        [2, 4]
    );
    snapshot.flush_pending(|x| Some(Shadow(*x.value))).unwrap();
    assert_eq!(c.shadows()[&2], None);
    assert_eq!(snapshot.shadows()[&2], Some(Shadow(2)));
    assert_eq!(snapshot.shadows()[&1], Some(Shadow(42)));
}
#[test]
fn flushed_changes_feed_incremental_stream_scores_and_cancelled_changes_do_not() {
    let p = problem();
    let a = p.initial_assignment();
    let mut cache = ListCascadeCache::new_deferred(&p, &a).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<i32, Option<i64>>();
    b.score("arrival", source.port(), |v| {
        SimpleScore::new(-v.unwrap_or(0))
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (&id, &value) in cache.shadows().iter() {
        tx.apply(&source, TupleChange::put(id, value)).unwrap();
    }
    tx.commit().unwrap();
    cache.clear_pending_updates();
    assert!(
        cache
            .flush_pending(|_| panic!("cleared"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(g.score(), SimpleScore::ZERO);
    cache.queue_update(&a, p.values()).unwrap();
    for next in [a, ListAssignment::new(vec![vec![4, 2], vec![3]])] {
        cache.queue_update(&next, &[]).unwrap();
        let changed = cache.flush_pending(arrival).unwrap();
        let mut tx = g.transaction();
        for id in changed {
            tx.apply(&source, TupleChange::put(id, cache.shadows()[&id]))
                .unwrap();
        }
        tx.commit().unwrap();
        assert_eq!(
            g.score(),
            SimpleScore::new(-full(&next).values().filter_map(|v| *v).sum::<i64>())
        );
    }
    g.assert_consistent().unwrap();
}

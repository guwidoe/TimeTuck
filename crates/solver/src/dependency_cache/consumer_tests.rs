//! Original structural work bounds; no upstream source-method credit.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn fanout(count: usize) -> DependencyCache<usize, i64> {
    let mut topology = DependencyTopology::new(0..count + 4).unwrap();
    let mut edges: Vec<_> = (3..count + 3).map(|target| (target, 0)).collect();
    edges.push((count + 3, 2));
    topology.update(&[], &edges).unwrap();
    DependencyCache::new(topology, |_, _| None)
}

#[test]
fn repeated_trials_share_every_surviving_consumer_from_the_original_checkpoint() {
    for count in [1, 8, 4096] {
        let saved = fanout(count);
        let original = &saved.plan.consumers[&0];
        assert_eq!(original.len(), count);
        for target in [3, count / 2 + 3, count + 2] {
            let mut trial = saved.clone();
            let mut calls = vec![];
            assert!(
                trial
                    .update(&[(target, 0)], &[(target, 1)], &[], |key, inputs| {
                        calls.push(*key);
                        assert_eq!(inputs.iter().map(|(k, _)| **k).collect::<Vec<_>>(), [1]);
                        None
                    })
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(calls, [target]);
            assert!(Arc::ptr_eq(&saved.plan.order, &trial.plan.order));
            assert!(saved.values.shares_root(&trial.values));
            assert!(saved.plan.consumers[&2].shares_root(&trial.plan.consumers[&2]));
            assert_eq!(
                trial.plan.consumers[&1].keys().copied().collect::<Vec<_>>(),
                [target]
            );
            if count == 1 {
                assert!(!trial.plan.consumers.contains_key(&0));
            } else {
                let survivors = &trial.plan.consumers[&0];
                assert_eq!(survivors.len(), count - 1);
                let shared = original
                    .keys()
                    .filter(|&&key| key != target)
                    .zip(survivors.keys())
                    .filter(|(before, after)| std::ptr::eq(*before, *after))
                    .count();
                // Existing entries retain their exact allocations; the former
                // BTreeSet clone copied all `count` IDs on every such trial.
                assert_eq!(shared, count - 1);
            }
            assert_eq!(original.len(), count);
        }
    }
}

#[test]
fn batched_edits_remove_empty_sets_and_failure_keeps_exact_roots() {
    let mut cache = fanout(128);
    let saved = cache.clone();
    let removals: Vec<_> = (3..131).map(|target| (target, 0)).collect();
    let additions: Vec<_> = (3..131).rev().map(|target| (target, 1)).collect();
    let mut calls = vec![];
    cache
        .update(&removals, &additions, &[], |key, _| {
            calls.push(*key);
            None
        })
        .unwrap();
    assert_eq!(calls, (3..131).collect::<Vec<_>>());
    assert!(!cache.plan.consumers.contains_key(&0));
    assert_eq!(cache.plan.consumers[&1].len(), 128);
    assert_eq!(saved.plan.consumers[&0].len(), 128);
    let current = cache.clone();
    let mut failure_calls = 0;
    let failure: Result<_, DependencyError<usize>> =
        cache.try_update(&additions, &removals, &[], |_, _| {
            failure_calls += 1;
            Err(DependencyError::UnknownNode(999))
        });
    assert_eq!(failure, Err(DependencyError::UnknownNode(999)));
    assert_eq!(failure_calls, 1);
    assert!(Arc::ptr_eq(&cache.plan, &current.plan));
    assert!(cache.values.shares_root(&current.values));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cache.update(&additions, &removals, &[], |_, _| panic!("supplier"))
        }))
        .is_err()
    );
    assert!(Arc::ptr_eq(&cache.plan, &current.plan));
    assert!(cache.values.shares_root(&current.values));
    cache
        .update(&additions, &removals, &[], |_, _| None)
        .unwrap();
    assert_eq!(cache.topology(), saved.topology());
    assert!(
        cache.plan.consumers[&0]
            .keys()
            .eq(saved.plan.consumers[&0].keys())
    );
}

//! Original checked arithmetic, atomicity, graph and search regressions.
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn all_arithmetic_boundaries_match_widened_integer_operations() {
    let edge = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    for a in edge {
        for b in edge {
            let mut c = SumCollector::default();
            c.try_insert(a).unwrap();
            let saved = c.clone();
            let expected = i64::try_from(i128::from(a) + i128::from(b)).map_err(|_| SumOverflow);
            assert_eq!(c.try_insert(b).map(|()| c.finish()), expected);
            if expected.is_err() {
                assert_eq!(c, saved);
            }
            let mut c = saved.clone();
            let expected = i64::try_from(i128::from(b) - i128::from(a))
                .map(|_| b)
                .map_err(|_| SumOverflow);
            assert_eq!(
                c.try_replace(a, b).map(|found| {
                    assert!(found);
                    c.finish()
                }),
                expected
            );
            if expected.is_err() {
                assert_eq!(c, saved);
            }
        }
    }
}
#[test]
fn duplicate_zero_and_missing_captures_and_both_replacement_overflows_are_atomic() {
    let mut c = SumCollector::default();
    for v in [-1, i64::MAX, 1, 0, 0] {
        c.try_insert(v).unwrap();
    }
    let saved = c.clone();
    assert_eq!(c.try_remove(-1), Err(SumOverflow));
    assert_eq!(c, saved);
    assert_eq!(c.try_replace(1, 2), Err(SumOverflow));
    assert_eq!(c, saved);
    assert_eq!(c.try_replace(-1, -1), Ok(true));
    assert_eq!(c, saved);
    assert_eq!(c.try_replace(-1, -2), Ok(true));
    assert_eq!(c.finish(), i64::MAX - 1);
    assert_eq!(c.try_remove(99), Ok(false));
    assert_eq!(c.try_replace(99, i64::MIN), Ok(false));
    for v in [0, 0, 1, i64::MAX, -2] {
        assert_eq!(c.try_remove(v), Ok(true));
    }
    assert_eq!(c.finish(), 0);
    assert_eq!(c.try_remove(0), Ok(false));
    let mut c = SumCollector::default();
    c.try_insert(i64::MIN).unwrap();
    let snapshot = c.clone();
    assert_eq!(c.try_replace(i64::MIN, i64::MAX), Err(SumOverflow));
    assert_eq!(c, snapshot);
    assert_eq!(c.try_remove(i64::MIN), Ok(true));
    assert_eq!(c.finish(), 0);
}
#[test]
fn map_and_group_updates_retain_zero_keys_and_restore_failed_total() {
    type Map = MapCollector<u8, i64, SumCollector>;
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, i64)>();
    let p = b.collect::<_, _, _, Map>(s.port(), |r| *r).unwrap();
    let h = b
        .score("maps", &p, |m| {
            SimpleScore::new(m.values().fold(0i64, |a, v| a.wrapping_add(*v)))
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261227u64;
    for step in 0..600 {
        let saved = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 24) as usize;
            let value =
                (!draw().is_multiple_of(4)).then(|| ((draw() % 4) as u8, (draw() % 11) as i64 - 5));
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut expected = BTreeMap::<u8, i64>::new();
            for &(k, v) in next.values() {
                *expected.entry(k).or_default() += v;
            }
            let matches = t.graph().unwrap().constraint_matches(&h).unwrap();
            if next.is_empty() {
                assert!(matches.is_empty());
            } else {
                let actual = matches
                    .values()
                    .next()
                    .unwrap()
                    .0
                    .iter()
                    .map(|(&k, &v)| (k, v))
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(actual, expected);
            }
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(expected.values().sum())
            );
        }
        if step % 17 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        } else {
            t.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn graph_sum_overflow_invalidates_trial_and_restores_checkpoint() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let p = b
        .collect::<_, _, _, SumCollector>(s.port(), |v| *v)
        .unwrap();
    let h = b.score("sum", &p, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, v) in [-1, i64::MAX, 1].into_iter().enumerate() {
        t.apply(&s, TupleChange::put(id, v)).unwrap();
    }
    t.commit().unwrap();
    let saved = g.constraint_matches(&h).unwrap().clone();
    for change in [
        TupleChange::retract(0),
        TupleChange::put(2, 2),
        TupleChange::put(3, 1),
    ] {
        let mut t = g.transaction();
        assert!(catch_unwind(AssertUnwindSafe(|| t.apply(&s, change))).is_err());
        assert!(t.score().is_err());
        drop(t);
        assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        g.assert_consistent().unwrap();
    }
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, -1)).unwrap();
    assert_eq!(t.score().unwrap(), SimpleScore::new(i64::MAX));
    t.commit().unwrap();
}
#[test]
fn checked_sum_scores_match_independent_search_and_construction() {
    let p = FiniteProblem::new(vec![Variable::new(vec![-2i64, 1, 3]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("sum", (0..4).collect(), |v| {
        let sum = v.iter().filter_map(|v| v.copied()).sum::<i64>();
        SimpleScore::new(-sum * sum)
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let c = b
            .collect_conditionally::<_, _, _, SumCollector>(
                s.port(),
                |r| r.1.is_some(),
                |r| r.1.unwrap(),
            )
            .unwrap();
        b.score("sum", &c, |v| SimpleScore::new(-v * v)).unwrap();
        (b.build(), s)
    };
    let initial = Assignment::new(vec![Some(-2); 4]);
    for cap in 1..=32 {
        let cfg = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&p, &n, initial.clone(), cfg.clone()).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&p, g, s, initial.clone(), cfg).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
    }
    let cfg = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let (g, s) = factory();
    let actual = greedy_stream_construction(&p, g, s, cfg).unwrap();
    let expected = greedy_construction(&p, &n, cfg).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
}

// Original large-tree persistence and overflow controls.

#[test]
fn large_capture_snapshots_survive_replace_remove_and_reinsert() {
    let mut collector = SumCollector::default();
    for value in -256..256 {
        collector.try_insert(value).unwrap();
    }
    // Zero remains a captured occurrence although it does not change the sum.
    collector.try_insert(0).unwrap();
    let initial = collector.clone();
    let mut expected = -256;
    for value in -256..256 {
        let before = collector.clone();
        assert_eq!(collector.try_replace(value, value + 1024), Ok(true));
        expected += 1024;
        assert_eq!(before.finish(), expected - 1024);
        assert_eq!(collector.finish(), expected);
        if value != 0 {
            assert_eq!(collector.try_remove(value), Ok(false));
        }
        assert_eq!(initial.finish(), -256);
    }
    for value in 768..1280 {
        assert_eq!(collector.try_remove(value), Ok(true));
        expected -= value;
        assert_eq!(collector.finish(), expected);
    }
    assert_eq!(collector.try_remove(0), Ok(true));
    assert_eq!(collector.try_remove(0), Ok(false));
    assert_eq!(collector.finish(), 0);
    for value in -256..256 {
        collector.try_insert(value).unwrap();
    }
    collector.try_insert(0).unwrap();
    assert_eq!(collector, initial);
}

#[test]
fn tree_overflow_missing_and_equal_replacements_preserve_captures() {
    let mut collector = SumCollector::default();
    for value in [-3, -2, -1, 0, 1, 2, 3, i64::MAX] {
        collector.try_insert(value).unwrap();
    }
    let initial = collector.clone();
    assert_eq!(collector.try_insert(1), Err(SumOverflow));
    assert_eq!(collector.try_remove(-1), Err(SumOverflow));
    assert_eq!(collector.try_replace(0, 1), Err(SumOverflow));
    assert_eq!(collector.try_replace(-1, i64::MAX), Err(SumOverflow));
    assert_eq!(collector.try_replace(i64::MIN, i64::MAX), Ok(false));
    assert_eq!(collector.try_remove(-99), Ok(false));
    assert_eq!(collector.try_remove(99), Ok(false));
    assert_eq!(collector.try_replace(99, 100), Ok(false));
    assert_eq!(collector.try_replace(0, 0), Ok(true));
    assert_eq!(collector.try_replace(99, 99), Ok(false));
    assert_eq!(collector, initial);
    assert_eq!(collector.try_remove(i64::MAX), Ok(true));
    for value in [-3, -2, -1, 0, 1, 2, 3] {
        assert_eq!(collector.try_remove(value), Ok(true));
    }
    assert_eq!(collector.finish(), 0);
    assert_eq!(initial.finish(), i64::MAX);
}

// Original bounded deferred-edit/cancellation contracts. No upstream method credit.
#[test]
fn deferred_inverse_still_rejects_overflowing_delta_and_trait_panic_is_atomic() {
    let mut c = SumCollector::default();
    c.try_insert(0).unwrap();
    c.try_insert(0).unwrap();
    assert_eq!(c.try_replace(0, i64::MIN), Ok(true));
    assert_eq!(c.finish(), i64::MIN);
    let saved = c.clone();
    assert_eq!(c.try_replace(i64::MIN, 0), Err(SumOverflow));
    assert_eq!(c, saved);
    assert!(
        catch_unwind(AssertUnwindSafe(|| ReversibleCollector::replace(
            &mut c,
            &i64::MIN,
            &0
        )))
        .is_err()
    );
    assert_eq!(c, saved);
    assert_eq!(c.try_remove(i64::MIN), Ok(true));
    assert_eq!(c.finish(), 0);
    assert_eq!(c.try_remove(0), Ok(true));
    assert_eq!(c.try_remove(0), Ok(false));
    assert_eq!(saved.finish(), i64::MIN);
}
#[test]
fn deferred_logical_equality_clones_branches_and_flushes_ignore_representation() {
    fn collect(values: &[i64]) -> SumCollector {
        let mut c = SumCollector::default();
        for &v in values {
            c.try_insert(v).unwrap();
        }
        c
    }
    fn send_sync<T: Send + Sync>() {}
    send_sync::<SumCollector>();
    let base = collect(&[-2, -1, 0, 0, 3]);
    let mut c = base.clone();
    assert_eq!(c.try_replace(-2, 4), Ok(true));
    let saved = c.clone();
    let logical = collect(&[-1, 0, 0, 3, 4]);
    assert_eq!(c, logical);
    assert_eq!(logical, c);
    let mut materialized = c.clone();
    materialized.try_insert(0).unwrap();
    assert_eq!(materialized.try_remove(0), Ok(true));
    assert_eq!(c, materialized);
    let mut inverse = c.clone();
    assert_eq!(inverse.try_replace(4, -2), Ok(true));
    assert_eq!(inverse, base);
    assert_eq!(c.try_replace(-2, 8), Ok(false));
    assert_eq!(c.try_replace(4, 4), Ok(true));
    assert_eq!(c, saved);
    for v in [-1, 0, 0, 3, 4] {
        assert_eq!(materialized.try_remove(v), Ok(true));
    }
    assert_eq!(materialized, SumCollector::default());
    assert_eq!(saved, logical);
    assert_eq!(base.finish(), 0);
}
#[test]
fn pending_duplicate_membership_and_composed_chains_preserve_every_occurrence() {
    let mut c = SumCollector::default();
    for v in [1, 1, 0, 0, -2] {
        c.try_insert(v).unwrap();
    }
    let initial = c.clone();
    assert_eq!(c.try_replace(1, 0), Ok(true)); // Destination already present.
    let once = c.clone();
    assert_eq!(c.try_replace(1, 1), Ok(true)); // One base source remains.
    assert_eq!(c.try_replace(0, 3), Ok(true)); // Compose 1->0->3; other zero captures remain.
    assert_eq!(c.finish(), 2);
    assert_eq!(c.try_replace(3, 1), Ok(true));
    assert_eq!(c, initial);
    let mut drained = once.clone();
    for v in [1, 0, 0, 0, -2] {
        assert_eq!(drained.try_remove(v), Ok(true));
    }
    assert_eq!(drained, SumCollector::default());
    assert_eq!(once.finish(), -1);
    // A composed physical edit need not have a representable net i64 delta.
    // Each public step is independently valid; flushing must not revalidate net delta.
    let mut wide = SumCollector::default();
    wide.try_insert(i64::MIN).unwrap();
    assert_eq!(wide.try_replace(i64::MIN, -1), Ok(true));
    assert_eq!(wide.try_replace(-1, i64::MAX - 1), Ok(true));
    let saved = wide.clone();
    let mut reconstructed = SumCollector::default();
    reconstructed.try_insert(i64::MAX - 1).unwrap();
    assert_eq!(wide, reconstructed);
    assert_eq!(wide.try_replace(i64::MAX - 1, i64::MIN), Err(SumOverflow));
    assert_eq!(wide, saved);
    wide.try_insert(0).unwrap();
    assert_eq!(wide.try_remove(i64::MAX - 1), Ok(true));
    assert_eq!(wide.finish(), 0);
    assert_eq!(wide.try_remove(0), Ok(true));
}
#[test]
fn pending_errors_missing_and_equal_operations_precede_materialization() {
    let mut c = SumCollector::default();
    for v in [-1, i64::MAX, 0] {
        c.try_insert(v).unwrap();
    }
    assert_eq!(c.try_replace(0, 1), Ok(true));
    let saved = c.clone();
    assert_eq!(c.try_insert(1), Err(SumOverflow));
    assert_eq!(c, saved);
    assert_eq!(c.try_remove(-1), Err(SumOverflow));
    assert_eq!(c, saved);
    assert_eq!(c.try_replace(1, 2), Err(SumOverflow));
    assert_eq!(c, saved);
    assert_eq!(c.try_replace(99, i64::MIN), Ok(false));
    assert_eq!(c.try_remove(i64::MIN), Ok(false));
    assert_eq!(c.try_replace(1, 1), Ok(true));
    assert_eq!(c.try_replace(0, 0), Ok(false));
    assert_eq!(c, saved);
    c.try_insert(0).unwrap();
    for v in [i64::MAX, -1, 1, 0] {
        assert_eq!(c.try_remove(v), Ok(true));
    }
    assert_eq!(c, SumCollector::default());
    assert_eq!(saved.finish(), i64::MAX);
}
#[test]
fn independently_reconstructed_mixed_histories_match_all_logical_frequencies() {
    fn reconstruct(model: &BTreeMap<i64, usize>) -> SumCollector {
        let mut c = SumCollector::default();
        for (&v, &n) in model {
            for _ in 0..n {
                c.try_insert(v).unwrap();
            }
        }
        c
    }
    let mut c = SumCollector::default();
    let mut model = BTreeMap::<i64, usize>::new();
    let mut snapshots = Vec::new();
    let mut seed = 299u64;
    for step in 0..1800 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let old = ((seed >> 32) % 25) as i64 - 12;
        let new = ((seed >> 16) % 25) as i64 - 12;
        match seed % 4 {
            0 => {
                c.try_insert(old).unwrap();
                *model.entry(old).or_default() += 1;
            }
            1 => {
                let present = model.contains_key(&old);
                assert_eq!(c.try_remove(old), Ok(present));
                if present {
                    let n = model.get_mut(&old).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        model.remove(&old);
                    }
                }
            }
            _ => {
                let present = model.contains_key(&old);
                assert_eq!(c.try_replace(old, new), Ok(present));
                if present {
                    let n = model.get_mut(&old).unwrap();
                    *n -= 1;
                    if *n == 0 {
                        model.remove(&old);
                    }
                    *model.entry(new).or_default() += 1;
                }
            }
        }
        assert_eq!(
            c.finish(),
            model.iter().map(|(&v, &n)| v * n as i64).sum::<i64>()
        );
        let independent = reconstruct(&model);
        assert_eq!(c, independent);
        assert_eq!(independent, c);
        if step % 137 == 0 {
            snapshots.push((c.clone(), model.clone()));
        }
    }
    for (snapshot, expected) in snapshots {
        assert_eq!(snapshot, reconstruct(&expected));
    }
    for (v, n) in model {
        for _ in 0..n {
            assert_eq!(c.try_remove(v), Ok(true));
        }
        assert_eq!(c.try_remove(v), Ok(false));
    }
    assert_eq!(c, SumCollector::default());
}

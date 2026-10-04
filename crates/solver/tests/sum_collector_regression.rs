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
        let expected = local_search_from(&p, &n, initial.clone(), cfg).unwrap();
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

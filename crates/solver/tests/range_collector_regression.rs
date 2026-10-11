//! Original native range regressions; no upstream method credit.
mod range_support;
use range_support::*;
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn identity_multiplicity_atomic_failures_and_immutable_hashes() {
    use std::hash::{Hash, Hasher};
    let hash = |c: &Chain| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        c.hash(&mut h);
        h.finish()
    };
    let mut c = C::default();
    let a = row(0, 0, 2);
    let b = row(1, 4, 6);
    for r in [a, a, b] {
        c.insert(&r);
    }
    let saved = c.finish();
    let before_hash = hash(&saved);
    for bad in [row(0, 0, 3), row(9, 8, 7)] {
        assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&a, &bad))).is_err());
        assert_eq!(c.finish(), saved);
    }
    assert!(!c.replace(&row(8, 0, 0), &row(7, 0, 1)));
    assert!(!c.remove(&row(8, 0, 0)));
    assert!(c.remove(&row(0, -99, 99)));
    verify(&c, &[a, b]);
    assert!(c.replace(&a, &row(0, 2, 4)));
    verify(&c, &[row(0, 2, 4), b]);
    assert_eq!(hash(&saved), before_hash);
    check(&saved, &reference(vec![a, a, b]));
    for id in [0, 1] {
        assert!(c.remove(&row(id, 0, 0)));
    }
    verify(&c, &[]);
}
#[test]
fn empty_intervals_custom_metrics_and_exact_integer_extremes() {
    let mut c = C::default();
    for r in [row(0, 0, 2), row(1, 1, 1), row(2, 2, 2)] {
        c.insert(&r);
    }
    assert!(!c.finish().ranges()[0].has_overlap());
    assert_eq!(c.finish().ranges()[0].maximum_overlap(), Some(1));
    c.remove(&row(0, 0, 2));
    let out = c.finish();
    assert_eq!(out.ranges().len(), 2);
    assert_eq!(out.ranges()[0].minimum_overlap(), None);
    let mut c = C::default();
    c.insert(&row(0, i64::MIN, i64::MAX));
    assert_eq!(*c.finish().ranges()[0].length(), (1i128 << 64) - 1);
    #[derive(Clone, Default)]
    struct Seconds;
    impl RangeMetric<i64> for Seconds {
        type Distance = SignedDuration;
        fn distance(&self, a: &i64, b: &i64) -> SignedDuration {
            SignedDuration::from_seconds(b - a)
        }
    }
    let mut c = ConnectedRangeCollector::<i64, i64, (), Seconds>::default();
    c.insert(&(0, 0, 86400, ()));
    c.insert(&(1, 2 * 86400, 3 * 86400, ()));
    assert_eq!(
        *c.finish().ranges()[0].length(),
        SignedDuration::from_seconds(86400)
    );
    assert_eq!(
        *c.finish().gaps()[0].length(),
        SignedDuration::from_seconds(86400)
    );
}
#[test]
fn generated_group_changes_match_independent_intervals_after_commit_and_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, Row)>();
    let p = b
        .group_collect_with::<_, _, _, _, C>(s.port(), |r| *r)
        .unwrap();
    let h = b
        .score("overlap", &p, |(_, c)| {
            SimpleScore::new(
                -(c.ranges()
                    .iter()
                    .map(|r| r.maximum_overlap().unwrap_or(0) as i64)
                    .sum::<i64>()),
            )
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261222u64;
    for step in 0..1000 {
        let old = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 24) as usize;
            let value = (!draw().is_multiple_of(4)).then(|| {
                let a = (draw() % 12) as i64 - 6;
                let end = a + (draw() % 5) as i64;
                ((draw() % 4) as u8, row(a * 100 + end, a, end))
            });
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, Vec<Row>>::new();
            for &(key, r) in next.values() {
                groups.entry(key).or_default().push(r);
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut score = 0;
            for (key, rs) in groups {
                let expected = reference(rs);
                check(&actual[&key].0.1, &expected);
                score -= expected
                    .iter()
                    .map(|r| bounds(r).1.unwrap_or(0) as i64)
                    .sum::<i64>();
            }
            assert_eq!(t.score().unwrap(), SimpleScore::new(score));
        }
        if step % 23 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&h).unwrap(), &old);
        } else {
            t.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn callback_failures_restore_graph_and_captured_retractions_do_not_map() {
    thread_local! {static FAIL_METRIC:Cell<bool>=const{Cell::new(false)};}
    #[derive(Clone, Default)]
    struct Metric;
    impl RangeMetric<i64> for Metric {
        type Distance = i128;
        fn distance(&self, a: &i64, b: &i64) -> i128 {
            assert!(!FAIL_METRIC.get());
            *b as i128 - *a as i128
        }
    }
    let fail = Cell::new(0);
    let mapped = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Row>();
    let p = b
        .collect::<_, _, _, ConnectedRangeCollector<i64, i64, (i64, i64), Metric>>(s.port(), |r| {
            assert_ne!(fail.get(), 1);
            mapped.set(mapped.get() + 1);
            *r
        })
        .unwrap();
    let h = b
        .score("ranges", &p, |c| {
            assert_ne!(fail.get(), 2);
            SimpleScore::new(c.ranges().len() as i64).with_structural(if fail.get() == 3 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, row(0, 0, 2))).unwrap();
    t.commit().unwrap();
    let old = g.constraint_matches(&h).unwrap().clone();
    for mode in 1..=4 {
        fail.set(mode);
        FAIL_METRIC.set(mode == 4);
        let mut t = g.transaction();
        let r = catch_unwind(AssertUnwindSafe(|| {
            t.apply(&s, TupleChange::put(0, row(0, 3, 6)))
        }));
        if mode == 3 {
            assert!(r.unwrap().is_err());
        } else {
            assert!(r.is_err());
        }
        assert!(t.score().is_err());
        drop(t);
        fail.set(0);
        FAIL_METRIC.set(false);
        assert_eq!(g.constraint_matches(&h).unwrap(), &old);
        assert_eq!(g.source_rows(&s).unwrap()[&0], row(0, 0, 2));
        g.assert_consistent().unwrap();
    }
    let calls = mapped.get();
    fail.set(1);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(mapped.get(), calls);
    assert_eq!(g.score(), SimpleScore::ZERO);
}
fn objective(values: impl Iterator<Item = (usize, i64)>) -> SimpleScore {
    SimpleScore::new(
        -reference(values.map(|(i, v)| row(i as i64, v, v + 2)).collect())
            .iter()
            .map(|r| {
                let n = bounds(r).1.unwrap_or(0) as i64;
                n * n
            })
            .sum::<i64>(),
    )
}
#[test]
fn construction_and_saved_search_candidates_match_full_range_objective() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 2, 4, 6]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("overlap", (0..4).collect(), |v| {
        objective(v.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i, *v))))
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let c = b
            .collect_conditionally::<_, _, _, C>(
                s.port(),
                |r| r.1.is_some(),
                |r| row(r.0 as i64, r.1.unwrap(), r.1.unwrap() + 2),
            )
            .unwrap();
        b.score("overlap", &c, |c| {
            SimpleScore::new(
                -c.ranges()
                    .iter()
                    .map(|r| {
                        let n = r.maximum_overlap().unwrap_or(0) as i64;
                        n * n
                    })
                    .sum::<i64>(),
            )
        })
        .unwrap();
        (b.build(), s)
    };
    let (g, s) = factory();
    let result = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(-1)));
    let initial = Assignment::new(vec![Some(0); 4]);
    for cap in 1..=32 {
        let config = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&p, &n, initial.clone(), config.clone()).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&p, g, s, initial.clone(), config).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
        let best = actual.best.unwrap();
        assert_eq!(
            best.score,
            objective(
                best.assignment
                    .values()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.map(|v| (i, v)))
            )
        );
    }
}

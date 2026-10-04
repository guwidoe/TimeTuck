//! Original load balance regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Row = (i32, i64, i64);
type C = LoadBalanceCollector<i32>;
fn loads(rows: impl Iterator<Item = Row>) -> BTreeMap<i32, BigInt> {
    let mut out = BTreeMap::new();
    for (k, v, initial) in rows {
        *out.entry(k).or_insert_with(|| BigInt::from(initial)) += v;
    }
    out
}
// Small bounded inputs: independently compute pairwise distances, then round
// their floating square root. Exact large boundaries have separate known checks.
fn unfairness(m: &BTreeMap<i32, BigInt>) -> Decimal {
    let v = m
        .values()
        .map(|v| i64::try_from(v).unwrap())
        .collect::<Vec<_>>();
    let mut pairs = 0i128;
    for i in 0..v.len() {
        for j in 0..i {
            let d = i128::from(v[i]) - i128::from(v[j]);
            pairs += d * d;
        }
    }
    if pairs == 0 {
        return Decimal::ZERO;
    }
    let root = (pairs as f64 / v.len() as f64).sqrt();
    let mut scale = 5 - root.log10().floor() as i32;
    let mut coefficient = (root * 10f64.powi(scale)).round() as i64;
    while coefficient % 10 == 0 {
        coefficient /= 10;
        scale -= 1;
    }
    Decimal::new(coefficient.into(), scale)
}
fn penalty(u: &Decimal) -> i64 {
    i64::try_from(u.coefficient()).unwrap() * 10i64.pow((6 - u.scale()) as u32)
}
fn check(out: &LoadBalance<i32>, rows: impl Iterator<Item = Row>) {
    let m = loads(rows);
    assert_eq!(out.loads(), &m);
    assert_eq!(out.unfairness(), &unfairness(&m));
}
#[test]
fn captures_initial_loads_zero_keys_atomic_replacement_and_owned_snapshots() {
    let mut c = C::default();
    let a = (0, 2, 10);
    let b = (0, -12, 10);
    let z = (1, 0, 0);
    for r in [a, a, b, z] {
        c.insert(&r);
    }
    check(&c.finish(), [a, a, b, z].into_iter());
    let saved = c.finish();
    assert_eq!(saved.loads()[&0], 2.into());
    assert_eq!(saved.loads()[&1], 0.into());
    assert!(!c.remove(&(0, 2, 11)));
    assert!(!c.replace(&(9, 0, 0), &a));
    for r in [(0, 3, 11), (0, 2, -1)] {
        assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&r))).is_err());
        assert_eq!(c.finish(), saved);
        assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&a, &r))).is_err());
        assert_eq!(c.finish(), saved);
    }
    assert!(c.remove(&a));
    assert!(c.remove(&b));
    assert_eq!(c.finish().loads()[&0], 12.into());
    assert!(c.replace(&a, &(0, 7, 20)));
    check(&c.finish(), [(0, 7, 20), z].into_iter());
    assert_eq!(saved.loads()[&0], 2.into());
    assert!(c.remove(&(0, 7, 20)));
    assert!(c.remove(&z));
    assert!(c.finish().loads().is_empty());
    assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
}
#[test]
fn exact_loads_avoid_overflow_and_large_offset_cancellation() {
    let mut c = C::default();
    c.insert(&(0, 1_000_000_000, 0));
    c.insert(&(1, 1_000_000_001, 0));
    assert_eq!(c.finish().unfairness(), &Decimal::new(707107.into(), 6));
    let mut c = C::default();
    c.insert(&(0, i64::MAX, i64::MAX));
    c.insert(&(0, i64::MAX, i64::MAX));
    c.insert(&(1, i64::MIN, i64::MIN));
    c.insert(&(1, i64::MIN, i64::MIN));
    assert_eq!(c.finish().loads()[&0], BigInt::from(i64::MAX) * 3);
    assert_eq!(c.finish().loads()[&1], BigInt::from(i64::MIN) * 3);
    // sqrt(((3*(2^64-1))^2)/2), rounded independently with Python Decimal.
    assert_eq!(c.finish().unfairness(), &Decimal::new(391315.into(), -14));
    let mut shifted = C::default();
    shifted.insert(&(0, i64::MAX, 0));
    shifted.insert(&(1, i64::MAX - 1, 0));
    assert_eq!(
        shifted.finish().unfairness(),
        &Decimal::new(707107.into(), 6)
    );
}
#[test]
fn grouped_edits_trials_and_rebuilds_match_pairwise_reference() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, Row)>();
    let p = b
        .group_collect_with::<_, _, _, _, C>(s.port(), |r| *r)
        .unwrap();
    let h = b
        .score("balance", &p, |(_, c)| {
            SimpleScore::new(-penalty(c.unfairness()))
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261223u64;
    for step in 0..600 {
        let old = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 20) as usize;
            let value = (!draw().is_multiple_of(4)).then(|| {
                let k = (draw() % 5) as i32;
                (
                    (draw() % 3) as u8,
                    (k, (draw() % 25) as i64 - 12, i64::from(k) * 3 - 4),
                )
            });
            if let Some(r) = value {
                next.insert(id, r);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, Vec<Row>>::new();
            for &(k, r) in next.values() {
                groups.entry(k).or_default().push(r);
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut score = 0;
            for (k, rs) in groups {
                check(&actual[&k].0.1, rs.clone().into_iter());
                score -= penalty(&unfairness(&loads(rs.into_iter())));
            }
            assert_eq!(t.score().unwrap(), SimpleScore::new(score));
        }
        if step % 17 == 0 {
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
fn failures_restore_graph_and_retraction_uses_captured_values() {
    let fail = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Row>();
    let p = b
        .collect::<_, _, _, C>(s.port(), |r| {
            assert_ne!(fail.get(), 1);
            calls.set(calls.get() + 1);
            *r
        })
        .unwrap();
    let h = b
        .score("balance", &p, |c| {
            assert_ne!(fail.get(), 2);
            SimpleScore::new(c.loads().len() as i64).with_structural(if fail.get() == 3 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..2 {
        t.apply(&s, TupleChange::put(id, (0, 2, 10))).unwrap();
    }
    t.commit().unwrap();
    let old = g.constraint_matches(&h).unwrap().clone();
    for mode in 1..=4 {
        fail.set(mode);
        let mut t = g.transaction();
        let r = catch_unwind(AssertUnwindSafe(|| {
            t.apply(
                &s,
                TupleChange::put(0, (0, 3, if mode == 4 { 11 } else { 10 })),
            )
        }));
        if mode == 3 {
            assert!(r.unwrap().is_err());
        } else {
            assert!(r.is_err());
        }
        assert!(t.score().is_err());
        drop(t);
        fail.set(0);
        assert_eq!(g.constraint_matches(&h).unwrap(), &old);
        g.assert_consistent().unwrap();
    }
    let before = calls.get();
    fail.set(1);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(calls.get(), before);
}
fn objective(values: impl Iterator<Item = i32>) -> SimpleScore {
    // Include all four resources via zero-load captures, even when idle.
    let m = loads((0..4).map(|k| (k, 0, 0)).chain(values.map(|k| (k, 1, 0))));
    let u = unfairness(&m);
    let scale = u.scale();
    let coefficient = i64::try_from(u.coefficient()).unwrap();
    SimpleScore::new(-coefficient * 10i64.pow((6 - scale) as u32))
}
#[test]
fn saved_search_candidates_match_full_load_balance_objective() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i32, 1, 2, 3]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i32, SimpleScore>::new(4);
    n.add_constraint("balance", (0..4).collect(), |v| {
        objective(v.iter().filter_map(|v| v.copied()))
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        let baseline = b.source::<usize, Row>();
        let tasks = b.filter(s.port(), |r| r.1.is_some()).unwrap();
        let tasks = b.map(&tasks, |r| (r.1.unwrap(), 1i64, 0i64)).unwrap();
        let all = b.concat(baseline.port(), &tasks).unwrap();
        let c = b.collect::<_, _, _, C>(&all, |r| *r).unwrap();
        b.score("balance", &c, |c| {
            let u = c.unfairness();
            SimpleScore::new(
                -i64::try_from(u.coefficient()).unwrap() * 10i64.pow((6 - u.scale()) as u32),
            )
        })
        .unwrap();
        let mut g = b.build();
        let mut t = g.transaction();
        for k in 0..4 {
            t.apply(&baseline, TupleChange::put(k, (k as i32, 0, 0)))
                .unwrap();
        }
        t.commit().unwrap();
        (g, s)
    };
    let initial = Assignment::new(vec![Some(0); 4]);
    for cap in 1..=32 {
        let config = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&p, &n, initial.clone(), config).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&p, g, s, initial.clone(), config).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
    }
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
    assert_eq!(result.score, Some(SimpleScore::ZERO));
}

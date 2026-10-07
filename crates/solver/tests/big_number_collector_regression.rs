//! Original arbitrary-precision, representation, graph and solver regressions.
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn d(value: i64, scale: i32) -> Decimal {
    Decimal::new(BigInt::from(value), scale)
}
fn rounded(value: i128, count: usize) -> i128 {
    let count = count as i128;
    let floor = value.div_euclid(count);
    let remainder = value.rem_euclid(count);
    if 2 * remainder < count || (2 * remainder == count && floor % 2 == 0) {
        floor
    } else {
        floor + 1
    }
}
#[test]
fn decimal_representation_numeric_order_and_extreme_scales_are_consistent() {
    let a = d(10, 1);
    let b = d(100, 2);
    assert_ne!(a, b);
    assert_eq!(a.numeric_cmp(&b), Ordering::Equal);
    assert!(a < b);
    assert_eq!(HashSet::from([a, b]).len(), 2);
    for scale in [i32::MIN, -1, 0, 1, i32::MAX] {
        let z = d(0, scale);
        assert_eq!(z.numeric_cmp(&d(0, 0)), Ordering::Equal);
        assert_eq!(z.add(&d(0, i32::MAX)), d(0, i32::MAX));
    }
    assert!(d(1, i32::MIN) > d(1, i32::MAX));
    assert!(d(-1, i32::MIN) < d(-1, i32::MAX));
    assert_eq!(d(1, i32::MIN).to_string(), "1e2147483648");
    let mut seed = 20261104u64;
    for _ in 0..5000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = (seed % 2_000_001) as i64 - 1_000_000;
        let sa = ((seed >> 20) % 9) as i32 - 4;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = (seed % 2_000_001) as i64 - 1_000_000;
        let sb = ((seed >> 20) % 9) as i32 - 4;
        let da = d(a, sa);
        let db = d(b, sb);
        let ia = a as i128 * 10i128.pow((4 - sa) as u32);
        let ib = b as i128 * 10i128.pow((4 - sb) as u32);
        assert_eq!(da.numeric_cmp(&db), ia.cmp(&ib));
        assert_eq!(da.cmp(&db), ia.cmp(&ib).then(sa.cmp(&sb)));
        let sum = da.add(&db);
        let diff = da.subtract(&db);
        assert_eq!(
            i128::try_from(sum.coefficient()).unwrap() * 10i128.pow((4 - sum.scale()) as u32),
            ia + ib
        );
        assert_eq!(
            i128::try_from(diff.coefficient()).unwrap() * 10i128.pow((4 - diff.scale()) as u32),
            ia - ib
        );
        assert_eq!(sum.scale(), sa.max(sb));
    }
}
#[test]
fn half_even_division_matches_independent_euclidean_reference() {
    for value in -100..=100 {
        for count in 1..=30 {
            for scale in [-2, 0, 3] {
                assert_eq!(
                    d(value, scale).divide_count(count),
                    Some(Decimal::new(
                        BigInt::from(rounded(value as i128, count)),
                        scale
                    ))
                );
            }
        }
    }
    assert_eq!(d(1, 0).divide_count(0), None);
    let huge = BigInt::from(10).pow(1000);
    for sign in [-1, 1] {
        let v = (&huge + 5) * sign;
        let quotient = (&huge / 2 + 2) * sign;
        assert_eq!(
            Decimal::from(v).divide_count(2),
            Some(Decimal::from(quotient))
        );
    }
}
#[test]
fn big_integer_duplicates_replacements_and_snapshots_are_exact() {
    let huge = BigInt::from(10).pow(1000) + 1;
    let mut c = BigIntSumCollector::default();
    c.insert(&huge);
    c.insert(&huge);
    let saved = c.clone();
    assert_eq!(c.finish(), &huge * 2);
    assert!(!c.remove(&BigInt::ZERO));
    assert_eq!(c, saved);
    assert!(c.replace(&huge, &(-&huge)));
    assert_eq!(c.finish(), BigInt::ZERO);
    assert_eq!(saved.finish(), &huge * 2);
    assert!(c.remove(&huge));
    assert_eq!(c.finish(), -&huge);
    assert!(c.remove(&(-&huge)));
    assert_eq!(c.finish(), BigInt::ZERO);
    assert!(!c.replace(&huge, &BigInt::ZERO));
}
#[test]
fn decimal_precision_history_and_exact_capture_survive_removal_and_empty_state() {
    let mut c = DecimalAverageCollector::default();
    c.insert(&d(7, 0));
    c.insert(&d(0, 0));
    assert_eq!(c.finish(), Some(d(4, 0)));
    let before = c.clone();
    c.insert(&d(0, 2));
    assert!(c.remove(&d(0, 2)));
    assert_eq!(c.finish(), Some(d(350, 2)));
    assert_eq!(before.finish(), Some(d(4, 0)));
    let saved = c.clone();
    assert!(!c.remove(&d(70, 1)));
    assert_eq!(c, saved);
    assert!(c.remove(&d(7, 0)));
    assert!(c.remove(&d(0, 0)));
    assert_eq!(c.finish(), None);
    c.insert(&d(7, 0));
    c.insert(&d(0, 0));
    assert_eq!(c.finish(), Some(d(350, 2)));
    let mut s = DecimalSumCollector::default();
    s.insert(&d(1, -3));
    assert_eq!(s.finish(), d(1000, 0));
    s.insert(&d(0, 2));
    assert!(s.remove(&d(0, 2)));
    assert!(s.remove(&d(1, -3)));
    assert_eq!(s.finish(), d(0, 2));
}
#[test]
fn nested_maps_and_pairs_keep_arbitrary_precision_captures() {
    type C = PairCollector<
        MapCollector<u8, BigInt, BigIntSumCollector>,
        MapCollector<u8, Decimal, DecimalAverageCollector>,
    >;
    let mut c = C::default();
    let large = BigInt::from(10).pow(100);
    let old = ((0, large.clone()), (0, d(3, 0)));
    c.insert(&old);
    c.insert(&old);
    let saved = c.finish();
    let new = ((1, -&large), (1, d(-5, 1)));
    assert!(c.replace(&old, &new));
    let (sums, means) = c.finish();
    assert_eq!(sums[&0], large);
    assert_eq!(sums[&1], -&large);
    assert_eq!(means[&0], Some(d(3, 0)));
    assert_eq!(means[&1], Some(d(-5, 1)));
    assert_eq!(saved.0[&0], &large * 2);
    assert!(c.remove(&old));
    assert!(c.remove(&new));
    let (sums, means) = c.finish();
    assert!(sums.is_empty());
    assert!(means.is_empty());
}
#[test]
fn generated_fixed_scale_graph_updates_match_independent_full_totals() {
    type C =
        PairCollector<PairCollector<BigIntSumCollector, CountCollector>, DecimalAverageCollector>;
    let large = BigInt::from(10).pow(80);
    let base = large.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, i64)>();
    let groups = b
        .group_collect_with::<_, _, _, _, C>(source.port(), move |(key, v)| {
            (*key, ((&base + *v, ()), d(*v, 2)))
        })
        .unwrap();
    let score_base = large.clone();
    let terminal = b
        .score("precise", &groups, move |(_, ((sum, count), average))| {
            SimpleScore::new(
                i64::try_from(sum - &score_base * *count).unwrap()
                    + i64::try_from(average.as_ref().unwrap().coefficient()).unwrap(),
            )
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261105u64;
    for step in 0usize..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = ((seed >> 32) % 40) as usize;
        let value = (!step.is_multiple_of(5))
            .then_some((((seed >> 12) % 4) as u32, ((seed >> 20) % 101) as i64 - 50));
        let mut candidate = rows.clone();
        if let Some(v) = value {
            candidate.insert(id, v);
        } else {
            candidate.remove(&id);
        }
        let saved = g.constraint_matches(&terminal).unwrap().clone();
        let mut t = g.transaction();
        t.apply(&source, TupleChange { id, value }).unwrap();
        let mut expected = BTreeMap::<u32, Vec<i64>>::new();
        for &(key, v) in candidate.values() {
            expected.entry(key).or_default().push(v);
        }
        let matches = t.graph().unwrap().constraint_matches(&terminal).unwrap();
        assert_eq!(matches.len(), expected.len());
        let mut score = 0;
        for (&key, values) in &expected {
            let sum = values.iter().map(|v| *v as i128).sum::<i128>();
            let mean = rounded(sum, values.len());
            assert_eq!(
                matches[&key].0.1,
                (
                    (&large * values.len() + BigInt::from(sum), values.len()),
                    Some(Decimal::new(BigInt::from(mean), 2))
                )
            );
            score += (sum + mean) as i64;
        }
        assert_eq!(t.score().unwrap(), SimpleScore::new(score));
        if step % 50 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&terminal).unwrap(), &saved);
        } else {
            t.commit().unwrap();
            rows = candidate;
        }
    }
}
#[test]
fn mixed_scale_rebuild_reports_history_difference_without_losing_cached_state() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Decimal>();
    let mean = b
        .collect::<_, _, _, DecimalAverageCollector>(source.port(), Clone::clone)
        .unwrap();
    let terminal = b
        .score("cents", &mean, |v| {
            let v = v.as_ref().unwrap();
            SimpleScore::new(
                i64::try_from(v.coefficient() * BigInt::from(10).pow((2 - v.scale()) as u32))
                    .unwrap(),
            )
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, v) in [(0, d(7, 0)), (1, d(0, 0)), (2, d(0, 2))] {
        t.apply(&source, TupleChange::put(id, v)).unwrap();
    }
    t.apply(&source, TupleChange::retract(2)).unwrap();
    t.commit().unwrap();
    let saved = g.constraint_matches(&terminal).unwrap().clone();
    assert_eq!(g.score(), SimpleScore::new(350));
    assert!(matches!(
        g.assert_consistent(),
        Err(GraphError::CorruptedScore(_))
    ));
    assert_eq!(g.score(), SimpleScore::new(350));
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &saved);
}
#[test]
fn big_number_callback_panics_roll_back_and_solver_uses_exact_projection() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, BigInt>();
    let sum = b
        .collect::<_, _, _, BigIntSumCollector>(source.port(), Clone::clone)
        .unwrap();
    b.score("bounded", &sum, |v| {
        assert!(v < &BigInt::from(10));
        SimpleScore::new(i64::try_from(v).unwrap())
    })
    .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, BigInt::from(3)))
        .unwrap();
    t.commit().unwrap();
    let mut t = g.transaction();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || t.apply(&source, TupleChange::put(1, BigInt::from(10).pow(1000)))
        ))
        .is_err()
    );
    assert!(t.score().is_err());
    drop(t);
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
    let graph = || {
        let base = BigInt::from(10).pow(100);
        let score_base = base.clone();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let sum = b
            .collect::<_, _, _, PairCollector<BigIntSumCollector, CountCollector>>(
                &assigned,
                move |(_, v)| (&base + v.unwrap(), ()),
            )
            .unwrap();
        b.score("excess", &sum, move |(sum, count)| {
            SimpleScore::new(-i64::try_from(sum - &score_base * *count).unwrap())
        })
        .unwrap();
        (b.build(), source)
    };
    let p = FiniteProblem::new((0..4).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = graph();
    let c = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(c.complete);
    assert_eq!(c.score, Some(SimpleScore::ZERO));
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(2))
            .collect(),
    )
    .unwrap();
    let (g, s) = graph();
    let best = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            assert_scores: true,
            max_steps: 20,
            max_evaluations: 1000,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(best.score, SimpleScore::ZERO);
    assert_eq!(best.assignment.values(), &[Some(0); 4]);
}

//! Original duration arithmetic, composition and solver regressions.
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn d(nanos: i128) -> SignedDuration {
    SignedDuration::from_nanos(nanos).unwrap()
}
#[test]
fn signed_values_normalize_and_check_intermediate_seconds() {
    assert_eq!(d(-1).seconds(), -1);
    assert_eq!(d(-1).subsec_nanos(), 999_999_999);
    assert_eq!(SignedDuration::from_seconds(i64::MIN), SignedDuration::MIN);
    assert_eq!(SignedDuration::MAX.seconds(), i64::MAX);
    for n in [
        i128::MIN,
        SignedDuration::MIN.as_nanos() - 1,
        SignedDuration::MAX.as_nanos() + 1,
        i128::MAX,
    ] {
        assert_eq!(SignedDuration::from_nanos(n), Err(DurationOverflow));
    }
    assert_eq!(
        d(SignedDuration::MIN.as_nanos() + 1).checked_add(d(-1)),
        Err(DurationOverflow)
    );
    assert_eq!(
        d(SignedDuration::MIN.as_nanos() + 1).checked_sub(d(1)),
        Ok(SignedDuration::MIN)
    );
    assert_eq!(
        SignedDuration::ZERO.checked_sub(d(SignedDuration::MIN.as_nanos() + 1)),
        Ok(SignedDuration::MAX)
    );
    assert_eq!(
        SignedDuration::MIN.checked_sub(SignedDuration::MIN),
        Ok(SignedDuration::ZERO)
    );
    assert_eq!(
        SignedDuration::ZERO.checked_sub(SignedDuration::MIN),
        Err(DurationOverflow)
    );
    let mut seed = 20261101u64;
    for _ in 0..5000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = seed as i64 as i128;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = seed as i64 as i128;
        assert_eq!(d(a).checked_add(d(b)).unwrap().as_nanos(), a + b);
        assert_eq!(d(a).checked_sub(d(b)).unwrap().as_nanos(), a - b);
        assert_eq!(
            d(a).seconds() as i128 * 1_000_000_000 + d(a).subsec_nanos() as i128,
            a
        );
    }
}
#[test]
fn collector_failures_preserve_snapshots_and_average_finish_is_fallible() {
    let mut sum = DurationSumCollector::default();
    sum.try_insert(SignedDuration::MAX).unwrap();
    let saved = sum.clone();
    assert_eq!(sum.try_insert(d(1)), Err(DurationOverflow));
    assert_eq!(sum, saved);
    assert_eq!(sum.try_remove(d(1)), Ok(false));
    assert_eq!(sum.try_replace(d(1), SignedDuration::MIN), Ok(false));
    assert_eq!(sum, saved);
    assert!(
        sum.try_replace(SignedDuration::MAX, SignedDuration::MIN)
            .unwrap()
    );
    sum.try_insert(d(1)).unwrap();
    let saved = sum.clone();
    assert_eq!(
        sum.try_replace(SignedDuration::MIN, SignedDuration::MAX),
        Err(DurationOverflow)
    );
    assert_eq!(sum, saved);
    assert!(sum.try_remove(d(1)).unwrap());
    assert!(sum.try_remove(SignedDuration::MIN).unwrap());
    assert_eq!(sum.finish(), SignedDuration::ZERO);
    let mut mean = DurationAverageCollector::default();
    mean.try_insert(d(i64::MAX as i128)).unwrap();
    mean.try_insert(d(1)).unwrap();
    assert_eq!(mean.try_finish(), Err(DurationOverflow));
    assert!(catch_unwind(AssertUnwindSafe(|| mean.finish())).is_err());
    assert!(mean.try_remove(d(1)).unwrap());
    assert_eq!(mean.try_finish(), Ok(Some(d(i64::MAX as i128))));
    assert!(mean.try_replace(d(i64::MAX as i128), d(-3)).unwrap());
    mean.try_insert(d(0)).unwrap();
    assert_eq!(mean.finish(), Some(d(-1)));
    assert!(mean.try_remove(d(-3)).unwrap());
    assert!(mean.try_remove(d(0)).unwrap());
    assert_eq!(mean.finish(), None);
}
#[test]
fn nested_map_and_pair_finish_failure_preserves_group_checkpoint() {
    type C = PairCollector<
        MapCollector<u8, SignedDuration, DurationAverageCollector>,
        DurationSumCollector,
    >;
    let mut node = CollectNode::<usize, (), ((u8, SignedDuration), SignedDuration), C>::new();
    node.apply(TupleChange::put(0, ((), ((0, d(i64::MAX as i128)), d(0)))));
    let saved = node.groups().next().unwrap().1;
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply(TupleChange::put(1, ((), ((0, d(1)), d(5)))))
        ))
        .is_err()
    );
    assert_eq!(node.groups().next().unwrap().1, saved);
    node.apply(TupleChange::put(0, ((), ((1, d(-3)), d(9)))));
    let (m, sum) = node.groups().next().unwrap().1;
    assert_eq!(m.len(), 1);
    assert_eq!(m[&1], Some(d(-3)));
    assert_eq!(sum, d(9));
    assert_eq!(saved.0[&0], Some(d(i64::MAX as i128)));
    assert_eq!(saved.1, d(0));
    node.apply(TupleChange::retract(0));
    assert_eq!(node.groups().count(), 0);
}
#[test]
fn graph_rolls_back_update_and_finisher_panics() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, SignedDuration>();
    let means = b
        .collect::<_, _, _, DurationAverageCollector>(source.port(), |v| *v)
        .unwrap();
    let terminal = b
        .score("mean", &means, |v| {
            SimpleScore::new(v.unwrap().as_nanos() as i64)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, d(i64::MAX as i128)))
        .unwrap();
    t.commit().unwrap();
    let saved = g.constraint_matches(&terminal).unwrap().clone();
    for (id, value) in [
        (1, d(1)),
        (0, SignedDuration::MAX),
        (1, SignedDuration::MAX),
    ] {
        let mut t = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || t.apply(&source, TupleChange::put(id, value))
            ))
            .is_err()
        );
        assert!(t.score().is_err());
        drop(t);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &saved);
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
        g.assert_consistent().unwrap();
    }
}
#[test]
fn generated_duration_groups_match_independent_rebuild_with_commit_and_undo() {
    type C = PairCollector<DurationSumCollector, DurationAverageCollector>;
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, SignedDuration)>();
    let groups = b
        .group_collect_with::<_, _, _, _, C>(source.port(), |(key, v)| (*key, (*v, *v)))
        .unwrap();
    let terminal = b
        .score("time", &groups, |(_, (sum, mean))| {
            SimpleScore::new((sum.as_nanos() + mean.unwrap().as_nanos()) as i64)
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261102u64;
    for step in 0usize..3000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = ((seed >> 32) % 40) as usize;
        let value = (!step.is_multiple_of(5)).then_some((
            ((seed >> 12) % 4) as u32,
            d((seed >> 20) as i128 % 20_000_000_001 - 10_000_000_000),
        ));
        let mut candidate = rows.clone();
        if let Some(v) = value {
            candidate.insert(id, v);
        } else {
            candidate.remove(&id);
        }
        let saved = g.constraint_matches(&terminal).unwrap().clone();
        let mut t = g.transaction();
        t.apply(&source, TupleChange { id, value }).unwrap();
        let mut expected = BTreeMap::<u32, Vec<i128>>::new();
        for &(key, value) in candidate.values() {
            expected.entry(key).or_default().push(value.as_nanos());
        }
        let matches = t.graph().unwrap().constraint_matches(&terminal).unwrap();
        assert_eq!(matches.len(), expected.len());
        let mut score = 0;
        for (&key, values) in &expected {
            let sum = values.iter().sum::<i128>();
            let mean = sum / values.len() as i128;
            assert_eq!(matches[&key].0.1, (d(sum), Some(d(mean))));
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
fn signed_duration_objective_runs_construction_and_search() {
    let graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let mean = b
            .collect::<_, _, _, DurationAverageCollector>(&assigned, |(_, v)| {
                d((v.unwrap() as i128 - 1) * 1_000_000_000)
            })
            .unwrap();
        b.score("duration", &mean, |v| {
            SimpleScore::new(-(v.unwrap().as_nanos().abs() as i64))
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
    assert_eq!(
        best.assignment
            .values()
            .iter()
            .map(|v| v.unwrap() as i64 - 1)
            .sum::<i64>(),
        0
    );
}

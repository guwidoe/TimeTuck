//! Original numeric-boundary and replacement regressions; no upstream credit.
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn checked_operations_preserve_state_and_replacement_uses_pinned_boundaries() {
    let mut c = AverageCollector::default();
    assert_eq!(c.finish(), None);
    c.try_insert(i64::MAX).unwrap();
    let old = c.clone();
    assert_eq!(c.try_insert(1), Err(AverageOverflow));
    assert_eq!(c, old);
    assert_eq!(c.try_replace(i64::MAX, i64::MIN), Err(AverageOverflow));
    assert_eq!(c, old);
    assert_eq!(c.try_remove(-1), Ok(false));
    assert_eq!(c.try_replace(-1, i64::MAX), Ok(false));
    assert_eq!(c, old);
    c.try_insert(-1).unwrap();
    assert!(c.try_replace(-1, 1).unwrap());
    assert_eq!(
        c.finish().unwrap().to_bits(),
        ((i64::MIN as f64) / 2.0).to_bits()
    );
    let wrapped = c.clone();
    assert_eq!(c.try_remove(1), Err(AverageOverflow));
    assert_eq!(c, wrapped);
    let mut c = AverageCollector::default();
    for v in [1, i64::MIN, -1] {
        c.try_insert(v).unwrap();
    }
    let old = c.clone();
    assert_eq!(c.try_remove(1), Err(AverageOverflow));
    assert_eq!(c, old);
    assert!(c.try_replace(1, 2).unwrap());
    assert!(c.try_remove(i64::MIN).unwrap());
    assert_eq!(c.finish(), Some(0.5));
    assert!(c.try_remove(-1).unwrap());
    assert_eq!(c.finish(), Some(2.0));
    assert!(c.try_remove(2).unwrap());
    assert_eq!(c.finish(), None);
    for values in [
        [(1i64 << 53) + 1, 0, 0],
        [-((1i64 << 53) + 1), 0, 0],
        [1, -1, 0],
    ] {
        let mut c = AverageCollector::default();
        for v in values {
            c.try_insert(v).unwrap();
        }
        assert_eq!(
            c.finish().unwrap().to_bits(),
            (values.iter().sum::<i64>() as f64 / 3.0).to_bits()
        );
    }
}
#[test]
fn replacement_propagates_through_pairs_maps_and_captured_groups() {
    type Collector = PairCollector<MapCollector<u8, i64, AverageCollector>, CountCollector>;
    let mut node = CollectNode::<usize, (), ((u8, i64), ()), Collector>::new();
    for (id, v) in [1, i64::MIN, -1].into_iter().enumerate() {
        node.apply(TupleChange::put(id, ((), ((0, v), ()))));
    }
    let before = node.groups().next().unwrap().1;
    // Moving this value to another map key requires a checked removal and fails.
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply(TupleChange::put(0, ((), ((1, 2), ()))))
        ))
        .is_err()
    );
    assert_eq!(node.groups().next().unwrap().1, before);
    // Same-key replacement must be delegated, not implemented as remove+insert.
    node.apply(TupleChange::put(0, ((), ((0, 2), ()))));
    assert_eq!(node.groups().next().unwrap().1.1, 3);
    node.apply(TupleChange::retract(1));
    let (m, count) = node.groups().next().unwrap().1;
    assert_eq!(count, 2);
    assert_eq!(m[&0], Some(0.5));
    node.apply(TupleChange::retract(0));
    node.apply(TupleChange::retract(2));
    assert_eq!(node.groups().count(), 0);
}
#[test]
fn average_panics_invalidate_transactions_and_restore_cached_sources_and_matches() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let average = b
        .collect::<_, _, _, AverageCollector>(source.port(), |v| *v)
        .unwrap();
    let terminal = b
        .score("presence", &average, |_| SimpleScore::new(1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, i64::MAX)).unwrap();
    t.commit().unwrap();
    let old = g.constraint_matches(&terminal).unwrap().clone();
    for (id, v) in [(1, 1), (0, i64::MIN)] {
        let mut t = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || t.apply(&source, TupleChange::put(id, v))
            ))
            .is_err()
        );
        assert!(t.score().is_err());
        drop(t);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &old);
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
        g.assert_consistent().unwrap();
    }
}
#[test]
fn generated_group_means_match_full_reference_across_commit_and_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, i64)>();
    let averages = b
        .group_collect_with::<_, _, _, _, AverageCollector>(source.port(), |v| *v)
        .unwrap();
    let terminal = b
        .score("means", &averages, |(_, v)| {
            SimpleScore::new((v.unwrap() * 1024.0).round() as i64)
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261030u64;
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
        let mut groups = BTreeMap::<u32, Vec<i64>>::new();
        for &(k, v) in candidate.values() {
            groups.entry(k).or_default().push(v);
        }
        let expected = groups
            .iter()
            .map(|(&k, v)| (k, v.iter().sum::<i64>() as f64 / v.len() as f64))
            .collect::<BTreeMap<_, _>>();
        let actual = t.graph().unwrap().constraint_matches(&terminal).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (&k, &v) in &expected {
            assert_eq!(actual[&k].0.1.unwrap().to_bits(), v.to_bits());
        }
        assert_eq!(
            t.score().unwrap(),
            SimpleScore::new(expected.values().map(|v| (v * 1024.0).round() as i64).sum())
        );
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
fn average_score_runs_construction_and_local_search() {
    let graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let average = b
            .collect::<_, _, _, AverageCollector>(&assigned, |(_, v)| v.unwrap() as i64)
            .unwrap();
        b.score("average", &average, |v| {
            SimpleScore::new(-(v.unwrap() * 1000.0).round() as i64)
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
            max_evaluations: 1000,
            max_steps: 20,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(best.assignment.values(), &[Some(0); 4]);
    assert_eq!(best.score, SimpleScore::ZERO);
}

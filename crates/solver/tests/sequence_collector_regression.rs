//! Original native regressions; no upstream method credit.
mod sequence_support;
use sequence_support::{check, components};
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type C = SequenceCollector<i64, i64, i64>;

#[test]
fn identities_capture_removal_and_failed_replacement_without_changing_snapshots() {
    let mut c = C::default();
    for item in [(1, 2, 20), (1, 2, 20), (2, 2, 21), (3, 4, 40)] {
        c.insert(&item);
    }
    let saved = c.finish();
    check(&saved, &[vec![(1, 2, 20), (2, 2, 21)], vec![(3, 4, 40)]], 1);
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&(1, 3, 20)))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&(1, 2, 20), &(1, 2, 99)))).is_err());
    assert!(!c.replace(&(99, 0, 0), &(4, 3, 30)));
    assert_eq!(c.finish(), saved);
    assert!(c.remove(&(1, 999, 999))); // Captured identity, not a recomputed index.
    assert_eq!(c.finish(), saved);
    assert!(c.replace(&(1, 0, 0), &(1, 3, 30)));
    check(&c.finish(), &[vec![(2, 2, 21), (1, 3, 30), (3, 4, 40)]], 1);
    check(&saved, &[vec![(1, 2, 20), (2, 2, 21)], vec![(3, 4, 40)]], 1);
    for id in [1, 2, 3] {
        assert!(c.remove(&(id, 0, 0)));
    }
    assert!(!c.remove(&(1, 0, 0)));
    check(&c.finish(), &[], 1);
}

#[test]
fn exact_integer_boundaries_and_invalid_metric_configuration() {
    let mut c = C::default();
    c.insert(&(0, i64::MIN, 0));
    c.insert(&(1, i64::MAX, 1));
    assert_eq!(
        *c.finish().first_break().unwrap().length(),
        (1i128 << 64) - 1
    );
    assert_eq!(*c.finish().first_sequence().unwrap().length(), 1);
    let mut wide =
        SequenceCollector::<i64, i64, i64, IntegerSequenceMetric<{ u64::MAX }>>::default();
    wide.insert(&(0, i64::MIN, 0));
    wide.insert(&(1, i64::MAX, 1));
    assert_eq!(wide.finish().sequences().len(), 1);
    assert_eq!(
        *wide.finish().first_sequence().unwrap().length(),
        2 * ((1i128 << 64) - 1)
    );
    assert!(
        catch_unwind(SequenceCollector::<i64, i64, i64, IntegerSequenceMetric<0>>::default)
            .is_err()
    );
}

#[test]
fn generated_group_edits_preserve_full_fields_through_undo_and_rebuild() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, i64)>();
    let p = b
        .group_collect_with::<_, _, _, _, C>(s.port(), |r| (r.0, (r.1, r.1 / 2, r.1)))
        .unwrap();
    let h = b
        .score("runs", &p, |(_, c)| {
            SimpleScore::new(-(c.sequences().len() as i64))
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261220u64;
    for step in 0..1000 {
        let saved = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 24) as usize;
            let value = (!draw().is_multiple_of(4))
                .then(|| ((draw() % 4) as u8, (draw() % 20) as i64 - 10));
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, BTreeMap<i64, (i64, i64, i64)>>::new();
            for &(key, v) in next.values() {
                groups.entry(key).or_default().insert(v, (v, v / 2, v));
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut score = 0;
            for (k, items) in groups {
                let runs = components(items.into_values().collect(), 1);
                check(&actual[&k].0.1, &runs, 1);
                score -= runs.len() as i64;
            }
            assert_eq!(t.score().unwrap(), SimpleScore::new(score));
        }
        if step % 23 == 0 {
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
fn mapping_metric_and_downstream_failures_restore_graph_and_allow_reuse() {
    thread_local! {static FAIL_METRIC: Cell<bool> = const {Cell::new(false)};}
    #[derive(Clone, Default)]
    struct Metric;
    impl SequenceMetric<i64> for Metric {
        type Distance = i128;
        fn zero(&self) -> i128 {
            0
        }
        fn maximum_gap(&self) -> i128 {
            1
        }
        fn add(&self, a: i128, b: i128) -> i128 {
            a + b
        }
        fn distance(&self, a: &i64, b: &i64) -> i128 {
            assert!(!FAIL_METRIC.get());
            *b as i128 - *a as i128
        }
    }
    let fail = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (i64, i64)>();
    let p = b
        .collect::<_, _, _, SequenceCollector<i64, i64, i64, Metric>>(s.port(), |&(id, v)| {
            assert_ne!(fail.get(), 1);
            (id, v, v)
        })
        .unwrap();
    let h = b
        .score("runs", &p, |c| {
            assert_ne!(fail.get(), 2);
            SimpleScore::new(c.sequences().len() as i64).with_structural(if fail.get() == 3 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, (0, 1))).unwrap();
    t.commit().unwrap();
    let saved = g.constraint_matches(&h).unwrap().clone();
    for mode in 1..=4 {
        fail.set(mode);
        FAIL_METRIC.set(mode == 4);
        let mut t = g.transaction();
        let r = catch_unwind(AssertUnwindSafe(|| {
            t.apply(&s, TupleChange::put(1, (1, 3)))
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
        assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        assert_eq!(g.source_rows(&s).unwrap().len(), 1);
        g.assert_consistent().unwrap();
    }
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(1, (1, 2))).unwrap();
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&h).unwrap()[&()]
            .0
            .first_sequence()
            .unwrap()
            .count(),
        2
    );
}

fn objective(values: impl Iterator<Item = (usize, i64)>) -> SimpleScore {
    let runs = components(values.map(|(i, v)| (i as i64, v, v)).collect(), 1);
    SimpleScore::new(runs.iter().map(|r| (r.len() * r.len()) as i64).sum())
}
#[test]
fn construction_and_saved_search_candidates_match_independent_full_scoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![-2i64, 0, 1, 3]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("connected", (0..4).collect(), |v| {
        objective(v.iter().enumerate().filter_map(|(i, x)| x.map(|v| (i, *v))))
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let c = b
            .collect_conditionally::<_, _, _, C>(
                s.port(),
                |r| r.1.is_some(),
                |r| (r.0 as i64, r.1.unwrap(), r.1.unwrap()),
            )
            .unwrap();
        b.score("connected", &c, |c| {
            SimpleScore::new(
                c.sequences()
                    .iter()
                    .map(|r| (r.count() * r.count()) as i64)
                    .sum(),
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
    assert_eq!(result.score, Some(SimpleScore::new(16)));
    let initial = Assignment::new(vec![Some(-2), Some(0), Some(1), Some(3)]);
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

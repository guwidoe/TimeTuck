//! Original conditional-aggregation regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn captured_rejections_nullable_values_and_failed_operations_preserve_state() {
    let mut c = ConditionalCollector::<SetCollector<Option<i64>>>::default();
    assert!(!c.remove(&None));
    assert!(!c.replace(&None, &Some(None)));
    c.insert(&None);
    c.insert(&Some(None)); // Selected nullable payload, distinct from rejection.
    c.insert(&Some(Some(7)));
    let saved = c.finish();
    assert_eq!(
        saved.keys().copied().collect::<Vec<_>>(),
        vec![None, Some(7)]
    );
    assert!(c.replace(&None, &None));
    assert!(c.replace(&Some(None), &None));
    assert!(c.replace(&None, &Some(Some(8))));
    assert!(!c.replace(&Some(Some(99)), &None));
    assert!(!c.remove(&Some(Some(99))));
    assert_eq!(
        c.finish().keys().copied().collect::<Vec<_>>(),
        vec![Some(7), Some(8)]
    );
    assert!(c.remove(&None));
    assert!(!c.remove(&None));
    assert_eq!(
        saved.keys().copied().collect::<Vec<_>>(),
        vec![None, Some(7)]
    );
    assert!(c.remove(&Some(Some(7))));
    assert!(c.remove(&Some(Some(8))));
    assert!(c.finish().is_empty());

    let mut c = ConditionalCollector::<AverageCollector>::default();
    for v in [1, i64::MIN, -1] {
        c.insert(&Some(v));
    }
    let before = c.finish();
    // Checked removal fails, but delegated replacement succeeds. Replacing by
    // remove+insert would incorrectly reject this valid average operation.
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&Some(1), &None))).is_err());
    assert_eq!(c.finish(), before);
    assert!(!c.remove(&None));
    assert!(c.replace(&Some(1), &Some(2)));
    assert!(c.remove(&Some(i64::MIN)));
    assert_eq!(c.finish(), Some(0.5));
    let mut c = ConditionalCollector::<AverageCollector>::default();
    c.insert(&Some(i64::MAX));
    c.insert(&None);
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&None, &Some(1)))).is_err());
    assert_eq!(c.finish(), Some(i64::MAX as f64));
    assert!(c.remove(&None)); // Failed transition did not consume the rejection.
}

#[test]
fn graphs_retain_rejected_groups_and_capture_callbacks_for_retractions() {
    let keys = Cell::new(0);
    let predicates = Cell::new(0);
    let mappings = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, Option<i64>)>();
    let grouped = b
        .group_collect_conditionally::<_, _, _, _, IntCollector>(
            s.port(),
            |r| {
                keys.set(keys.get() + 1);
                r.0
            },
            |r| {
                predicates.set(predicates.get() + 1);
                r.1.is_some()
            },
            |r| {
                mappings.set(mappings.get() + 1);
                r.1.expect("rejected rows must not map")
            },
        )
        .unwrap();
    let c = b
        .score("group", &grouped, |(_, v)| SimpleScore::new(v.sum))
        .unwrap();
    let global = b
        .collect_conditionally::<_, _, _, CountCollector>(s.port(), |r| r.1.is_some(), |_| ())
        .unwrap();
    let all = b
        .score("global", &global, |v| SimpleScore::new(*v as i64))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, (1, None))).unwrap();
    t.commit().unwrap();
    assert_eq!(g.constraint_matches(&c).unwrap()[&1].0.1.count, 0);
    assert_eq!(g.constraint_matches(&all).unwrap()[&()].0, 0);
    assert_eq!((keys.get(), predicates.get(), mappings.get()), (1, 1, 0));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, (2, Some(7)))).unwrap();
    t.commit().unwrap();
    assert!(!g.constraint_matches(&c).unwrap().contains_key(&1));
    assert_eq!(g.score(), SimpleScore::new(8));
    assert_eq!((keys.get(), predicates.get(), mappings.get()), (2, 2, 1));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    assert!(
        t.graph()
            .unwrap()
            .constraint_matches(&c)
            .unwrap()
            .is_empty()
    );
    assert!(
        t.graph()
            .unwrap()
            .constraint_matches(&all)
            .unwrap()
            .is_empty()
    );
    drop(t);
    assert_eq!(g.score(), SimpleScore::new(8));
    assert_eq!((keys.get(), predicates.get(), mappings.get()), (2, 2, 1));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert_eq!((keys.get(), predicates.get(), mappings.get()), (2, 2, 1));
    g.assert_consistent().unwrap();
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
fn summaries(rows: &BTreeMap<usize, (u8, bool, i64)>) -> BTreeMap<u8, IntSummary> {
    let mut groups = BTreeMap::<u8, Vec<i64>>::new();
    for &(key, selected, value) in rows.values() {
        let values = groups.entry(key).or_default();
        if selected {
            values.push(value);
        }
    }
    groups
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                IntSummary {
                    count: v.len(),
                    distinct_count: v.iter().collect::<BTreeSet<_>>().len(),
                    sum: v.iter().sum(),
                    min: v.iter().copied().min(),
                    max: v.iter().copied().max(),
                },
            )
        })
        .collect()
}
#[test]
fn generated_group_transitions_match_independent_multisets_after_commit_and_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, bool, i64)>();
    type C = PairCollector<CountCollector, IntCollector>;
    let p = b
        .group_collect_conditionally::<_, _, _, _, C>(s.port(), |r| r.0, |r| r.1, |r| ((), r.2))
        .unwrap();
    let c = b
        .score("selected", &p, |(key, (count, sum))| {
            SimpleScore::new(*key as i64 + *count as i64 + sum.sum)
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261203;
    for step in 0..2000 {
        let saved = g.constraint_matches(&c).unwrap().clone();
        let before = g.score();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 48) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    (random(&mut seed) % 6) as u8,
                    random(&mut seed).is_multiple_of(2),
                    (random(&mut seed) % 21) as i64 - 10,
                )
            });
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let expected = summaries(&next);
            let actual = t.graph().unwrap().constraint_matches(&c).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (&k, summary) in &expected {
                let ((key, (count, s)), score) = &actual[&k];
                assert_eq!(*key, k);
                assert_eq!(*count, summary.count);
                assert_eq!(s, summary);
                assert_eq!(
                    *score,
                    SimpleScore::new(k as i64 + summary.count as i64 + summary.sum)
                );
            }
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(
                    expected
                        .iter()
                        .map(|(k, s)| *k as i64 + s.count as i64 + s.sum)
                        .sum()
                )
            );
        }
        if step % 29 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&c).unwrap(), &saved);
            assert_eq!(g.score(), before);
        } else {
            t.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}

#[test]
fn predicate_mapping_key_and_scoring_failures_restore_the_entire_graph() {
    let failure = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, i64)>();
    let p = b
        .group_collect_conditionally::<_, _, _, _, IntCollector>(
            s.port(),
            |r| {
                assert_ne!(failure.get(), 1, "key");
                r.0
            },
            |r| {
                assert_ne!(failure.get(), 2, "predicate");
                r.1 > 0
            },
            |r| {
                assert_ne!(failure.get(), 3, "mapping");
                r.1
            },
        )
        .unwrap();
    let c = b
        .score("conditional", &p, |(_, v)| {
            assert_ne!(failure.get(), 4, "scoring");
            SimpleScore::new(v.sum).with_structural(if failure.get() == 5 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, (1, 4))).unwrap();
    t.commit().unwrap();
    let old = g.constraint_matches(&c).unwrap().clone();
    for mode in 1..=5 {
        failure.set(mode);
        let mut t = g.transaction();
        let r = catch_unwind(AssertUnwindSafe(|| {
            t.apply(&s, TupleChange::put(0, (2, 8)))
        }));
        if mode == 5 {
            assert!(r.unwrap().is_err());
        } else {
            assert!(r.is_err());
        }
        assert!(t.score().is_err());
        drop(t);
        assert_eq!(g.source_rows(&s).unwrap()[&0], (1, 4));
        assert_eq!(g.constraint_matches(&c).unwrap(), &old);
        assert_eq!(g.score(), SimpleScore::new(4));
        failure.set(0);
        g.assert_consistent().unwrap();
    }
    // Foreign ports fail before adding an aggregation node or invoking callbacks.
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        b.group_collect_conditionally::<_, _, _, i64, IntCollector>(
            s.port(),
            |_| 0,
            |_| panic!("not called"),
            |_| panic!("not called")
        ),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.collect_conditionally::<_, _, i64, IntCollector>(
            s.port(),
            |_| panic!("not called"),
            |_| panic!("not called")
        ),
        Err(GraphError::ForeignPort)
    ));
    let s = b.source::<usize, i64>();
    let p = b
        .collect_conditionally::<_, _, _, IntCollector>(s.port(), |_| true, |v| *v)
        .unwrap();
    b.score("valid", &p, |v| SimpleScore::new(v.sum)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
}

#[test]
fn composed_conditional_collectors_preserve_all_input_group_membership() {
    type C = PairCollector<ConditionalCollector<IntCollector>, ConditionalCollector<IntCollector>>;
    let mut n = CollectNode::<usize, (), (Option<i64>, Option<i64>), C>::new();
    n.apply(TupleChange::put(0, ((), (None, None))));
    let (a, b) = n.groups().next().unwrap().1;
    assert_eq!((a.count, b.count), (0, 0));
    n.apply(TupleChange::put(1, ((), (Some(7), None))));
    n.apply(TupleChange::put(2, ((), (None, Some(-2)))));
    n.apply(TupleChange::put(0, ((), (Some(7), Some(3)))));
    let (a, b) = n.groups().next().unwrap().1;
    assert_eq!((a.count, a.distinct_count, a.sum), (2, 1, 14));
    assert_eq!((b.count, b.distinct_count, b.sum), (2, 2, 1));
    for id in [1, 2] {
        n.apply(TupleChange::retract(id));
    }
    n.apply(TupleChange::put(0, ((), (None, None))));
    assert_eq!(n.groups().count(), 1);
    assert_eq!(n.groups().next().unwrap().1.0.count, 0);
    n.apply(TupleChange::retract(0));
    assert_eq!(n.groups().count(), 0);
}

#[test]
fn construction_search_and_saved_candidates_match_full_conditional_objective() {
    let p = FiniteProblem::new(vec![Variable::new(vec![-2i64, -1, 0, 1, 2]); 4]).unwrap();
    let objective = |a: &Assignment<i64>| {
        SimpleScore::new(a.values().iter().flatten().map(|v| v.max(&0)).sum::<i64>())
    };
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("positive sum", (0..4).collect(), |v| {
        SimpleScore::new(v.iter().flatten().map(|v| (**v).max(0)).sum())
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let c = b
            .collect_conditionally::<_, _, _, IntCollector>(
                s.port(),
                |(_, v)| v.is_some_and(|v| v > 0),
                |(_, v)| v.unwrap(),
            )
            .unwrap();
        b.score("positive sum", &c, |v| SimpleScore::new(v.sum))
            .unwrap();
        (b.build(), s)
    };
    let (g, s) = factory();
    let constructed = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(constructed.score, Some(SimpleScore::new(8)));
    assert_eq!(constructed.score, Some(objective(&constructed.assignment)));
    let initial = Assignment::new(vec![Some(-2); 4]);
    for cap in 1..=24 {
        let config = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 6,
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
        assert_eq!(best.score, objective(&best.assignment));
    }
}

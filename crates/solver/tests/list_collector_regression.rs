//! Original collected-list regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn random(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    *s >> 32
}
#[test]
fn generated_occurrences_match_ordered_reference_and_rank_access() {
    let mut c = ListCollector::<usize, i64>::default();
    let mut expected = BTreeMap::new();
    let mut saved = Vec::new();
    let mut seed = 20261027;
    for step in 0..8000 {
        let id = (random(&mut seed) % 128) as usize;
        if let Some(old) = expected.remove(&id) {
            assert!(c.remove(&(id, old)));
        } else {
            assert!(!c.remove(&(id, 0)));
        }
        if !random(&mut seed).is_multiple_of(4) {
            let v = (random(&mut seed) % 11) as i64 - 5;
            c.insert(&(id, v));
            expected.insert(id, v);
        }
        let list = c.finish();
        let values = expected.values().copied().collect::<Vec<_>>();
        assert_eq!(list.len(), values.len());
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), values);
        for (i, v) in values.iter().enumerate() {
            assert_eq!(list.get(i), Some(v));
            assert_eq!(&list[i], v);
        }
        assert_eq!(list.get(values.len()), None);
        assert_eq!(list.get(usize::MAX), None);
        if step % 100 == 0 {
            saved.push((list, values));
        }
    }
    for (list, values) in saved {
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), values);
    }
}
fn weighted(values: impl Iterator<Item = i64>) -> i64 {
    values.enumerate().map(|(i, v)| (i as i64 + 1) * v).sum()
}
fn groups(rows: &BTreeMap<usize, (u32, i64)>) -> BTreeMap<u32, Vec<i64>> {
    let mut out = BTreeMap::<u32, Vec<i64>>::new();
    for &(k, v) in rows.values() {
        out.entry(k).or_default().push(v);
    }
    out
}
#[test]
fn global_and_grouped_lists_rebuild_after_nonmonotonic_ids_and_rollback() {
    let failure = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, i64)>();
    let all = b.collect_list(source.port(), |(_, v)| *v).unwrap();
    let keyed = b.group_list(source.port(), |v| *v).unwrap();
    let global = b
        .score("global order", &all, |v| {
            assert_ne!(failure.get(), 2, "scorer panic");
            SimpleScore::new(weighted(v.iter().copied())).with_structural(if failure.get() == 1 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let grouped = b
        .score("group order", &keyed, |(_, v)| {
            SimpleScore::new(weighted(v.iter().copied()))
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261028;
    for step in 0..1000 {
        let saved = g.constraint_matches(&global).unwrap().clone();
        let old_groups = g.constraint_matches(&grouped).unwrap().clone();
        let mut candidate = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 32) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    (random(&mut seed) % 4) as u32,
                    (random(&mut seed) % 17) as i64 - 8,
                )
            });
            if let Some(v) = value {
                candidate.insert(id, v);
            } else {
                candidate.remove(&id);
            }
            t.apply(&source, TupleChange { id, value }).unwrap();
            let actual = t.graph().unwrap().constraint_matches(&global).unwrap();
            if candidate.is_empty() {
                assert!(actual.is_empty());
            } else {
                assert_eq!(
                    actual[&()].0.iter().copied().collect::<Vec<_>>(),
                    candidate.values().map(|(_, v)| *v).collect::<Vec<_>>()
                );
            }
            let expected_groups = groups(&candidate);
            assert_eq!(
                t.graph()
                    .unwrap()
                    .constraint_matches(&grouped)
                    .unwrap()
                    .iter()
                    .map(|(&k, ((_, v), _))| (k, v.iter().copied().collect::<Vec<_>>()))
                    .collect::<BTreeMap<_, _>>(),
                expected_groups
            );
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(
                    weighted(candidate.values().map(|(_, v)| *v))
                        + expected_groups
                            .values()
                            .map(|v| weighted(v.iter().copied()))
                            .sum::<i64>()
                )
            );
        }
        if step % 25 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&global).unwrap(), &saved);
            assert_eq!(g.constraint_matches(&grouped).unwrap(), &old_groups);
        } else {
            t.commit().unwrap();
            rows = candidate;
        }
    }
    let old = g.constraint_matches(&global).unwrap().clone();
    let old_score = g.score();
    for mode in [1, 2] {
        failure.set(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(99, (9, 10)))?;
            t.commit()
        }));
        if mode == 1 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        failure.set(0);
        assert_eq!(g.score(), old_score);
        assert_eq!(g.constraint_matches(&global).unwrap(), &old);
        g.assert_consistent().unwrap();
    }
    let mut t = g.transaction();
    for &id in rows.keys() {
        t.apply(&source, TupleChange::retract(id)).unwrap();
    }
    t.commit().unwrap();
    assert!(g.constraint_matches(&global).unwrap().is_empty());
    assert!(g.constraint_matches(&grouped).unwrap().is_empty());
    assert_eq!(g.score(), SimpleScore::ZERO);
}
#[test]
fn composition_retains_equal_values_and_snapshot_equality_ignores_ids() {
    type ByTeam = MapCollector<&'static str, (usize, String), ListCollector<usize, String>>;
    let mut c = ByTeam::default();
    for (k, id, v) in [("a", 7, "same"), ("a", 2, "same"), ("b", 0, "other")] {
        c.insert(&(k, (id, v.into())));
    }
    let old = c.finish();
    let mut fork = c.clone();
    assert_eq!(
        old[&"a"].iter().map(String::as_str).collect::<Vec<_>>(),
        ["same", "same"]
    );
    assert!(c.remove(&("a", (2, "same".into()))));
    c.insert(&("a", (2, "changed".into())));
    assert_eq!(c.finish()[&"a"][0], "changed");
    assert_eq!(old[&"a"][0], "same");
    assert!(fork.remove(&("b", (0, "other".into()))));
    assert!(c.finish().contains_key(&"b"));
    let mut other = ListCollector::<usize, String>::default();
    other.insert(&(99, "same".into()));
    other.insert(&(100, "same".into()));
    assert_eq!(old[&"a"], other.finish());
    assert_eq!(format!("{:?}", other.finish()), "[\"same\", \"same\"]");
    let before = other.finish();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || other.insert(&(99, "duplicate ID".into()))
        ))
        .is_err()
    );
    assert_eq!(other.finish(), before);
    assert!(other.remove(&(99, "value is not compared".into())));
    assert_eq!(other.finish().len(), 1);
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ByTeam>();
    send_sync::<ListSnapshot<usize, String>>();
}
#[derive(Debug)]
struct Key(i32, Rc<Cell<Option<usize>>>);
impl Key {
    fn tick(&self) {
        if let Some(n) = self.1.get() {
            assert!(n > 0, "trait failure");
            self.1.set(Some(n - 1));
        }
    }
}
impl Clone for Key {
    fn clone(&self) -> Self {
        self.tick();
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Key {
    fn eq(&self, r: &Self) -> bool {
        self.cmp(r).is_eq()
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, r: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Key {
    fn cmp(&self, r: &Self) -> std::cmp::Ordering {
        self.tick();
        self.0.cmp(&r.0)
    }
}
#[test]
fn clone_and_ordering_panics_preserve_occurrences_and_old_snapshots() {
    let budget = Rc::new(Cell::new(None));
    let key = |n| Key(n, budget.clone());
    let mut c = ListCollector::<Key, Key>::default();
    for n in 0..16 {
        c.insert(&(key(n), key(n % 3)));
    }
    let old = c.finish();
    let read = |c: &ListCollector<Key, Key>| c.finish().iter().map(|v| v.0).collect::<Vec<_>>();
    let expected = read(&c);
    let mut failures = 0;
    for operation in 0..2 {
        let mut successes = 0;
        for allowance in 0..60 {
            let mut trial = c.clone();
            budget.set(Some(allowance));
            let result = catch_unwind(AssertUnwindSafe(|| {
                if operation == 0 {
                    trial.insert(&(key(20), key(9)));
                } else {
                    assert!(trial.remove(&(key(8), key(0))));
                }
            }));
            budget.set(None);
            if result.is_err() {
                failures += 1;
                assert_eq!(read(&trial), expected);
            } else {
                successes += 1;
                assert_eq!(trial.finish().len(), if operation == 0 { 17 } else { 15 });
            }
            assert_eq!(old.iter().map(|v| v.0).collect::<Vec<_>>(), expected);
            assert_eq!(read(&c), expected);
        }
        assert!(successes > 0);
    }
    assert!(failures > 10);
}
#[test]
fn ordered_list_scoring_integrates_with_construction_and_search() {
    let graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let list = b
            .collect_list(&assigned, |(_, v)| v.unwrap() as i64)
            .unwrap();
        b.score("ordered values", &list, |v| {
            SimpleScore::new(-weighted(v.iter().copied()))
        })
        .unwrap();
        (b.build(), source)
    };
    let p = FiniteProblem::new((0..4).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = graph();
    let built = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(built.complete);
    assert_eq!(built.score, Some(SimpleScore::ZERO));
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
    // Every mapped value is nonnegative; zero is a proven upper bound.
}

#[test]
fn list_value_equality_order_and_hash_support_downstream_distinct() {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let make = |base, values: &[i64]| {
        let mut c = ListCollector::<usize, i64>::default();
        for (i, &v) in values.iter().enumerate() {
            c.insert(&(base + i, v));
        }
        c.finish()
    };
    let hash = |v: &ListSnapshot<usize, i64>| {
        let mut h = DefaultHasher::new();
        v.hash(&mut h);
        h.finish()
    };
    let a = make(0, &[4, 5]);
    let b = make(100, &[4, 5]);
    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    assert_eq!(hash(&a), hash(&b));
    assert!(a < make(0, &[4, 6]));
    assert!(make(0, &[4]) < a);
    assert_eq!(ListSnapshot::<usize, i64>::default(), make(0, &[]));
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (u32, i64)>();
    let grouped = builder.group_list(source.port(), |v| *v).unwrap();
    let values = builder.map(&grouped, |(_, v)| v.clone()).unwrap();
    let distinct = builder.distinct(&values).unwrap();
    let terminal = builder
        .score("distinct lists", &distinct, |_| SimpleScore::new(1))
        .unwrap();
    let mut g = builder.build();
    let mut t = g.transaction();
    for (id, key, v) in [(40, 1, 5), (3, 0, 5), (20, 1, 4), (1, 0, 4)] {
        t.apply(&source, TupleChange::put(id, (key, v))).unwrap();
    }
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(1));
    assert_eq!(g.constraint_matches(&terminal).unwrap().len(), 1);
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(40, (1, 6))).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(2));
        t.assert_consistent().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(1));
    g.assert_consistent().unwrap();
}

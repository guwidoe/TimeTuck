//! Original map collector regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

type Inner = PairCollector<IntCollector, SetCollector<i64>>;
type Maps = MapCollector<u32, (i64, i64), Inner>;
fn reference(rows: &BTreeMap<usize, (u32, i64)>) -> BTreeMap<u32, (IntSummary, Vec<i64>)> {
    let mut groups = BTreeMap::<u32, Vec<i64>>::new();
    for &(k, v) in rows.values() {
        groups.entry(k).or_default().push(v);
    }
    groups
        .into_iter()
        .map(|(k, values)| {
            let set = values.iter().copied().collect::<BTreeSet<_>>();
            (
                k,
                (
                    IntSummary {
                        count: values.len(),
                        distinct_count: set.len(),
                        sum: values.iter().fold(0i64, |s, v| s.wrapping_add(*v)),
                        min: values.iter().copied().min(),
                        max: values.iter().copied().max(),
                    },
                    set.into_iter().collect(),
                ),
            )
        })
        .collect()
}
fn snapshot(
    m: &SnapshotMap<u32, (IntSummary, SnapshotMap<i64, ()>)>,
) -> BTreeMap<u32, (IntSummary, Vec<i64>)> {
    m.iter()
        .map(|(&k, (s, v))| (k, (*s, v.keys().copied().collect())))
        .collect()
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn captured_map_replacements_and_transactions_match_independent_multisets() {
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, i64)>();
    let maps = b
        .collect::<_, _, _, Maps>(source.port(), |(k, v)| (*k, (*v, *v)))
        .unwrap();
    let terminal = b
        .score("map weights", &maps, |m| {
            assert!(!fail.get(), "score panic");
            SimpleScore::new(
                m.values()
                    .fold(0i64, |total, (s, _)| total.wrapping_add(s.sum)),
            )
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261026;
    for step in 0..1500 {
        let saved = g.constraint_matches(&terminal).unwrap().clone();
        let mut candidate = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 40) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                let k = (random(&mut seed) % 7) as u32;
                let v = match random(&mut seed) % 20 {
                    0 => i64::MAX,
                    1 => i64::MIN,
                    _ => (random(&mut seed) % 11) as i64 - 5,
                };
                (k, v)
            });
            if let Some(v) = value {
                candidate.insert(id, v);
            } else {
                candidate.remove(&id);
            }
            t.apply(&source, TupleChange { id, value }).unwrap();
            let actual = t.graph().unwrap().constraint_matches(&terminal).unwrap();
            if candidate.is_empty() {
                assert!(actual.is_empty());
            } else {
                assert_eq!(snapshot(&actual[&()].0), reference(&candidate));
            }
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(
                    candidate
                        .values()
                        .fold(0i64, |s, (_, v)| s.wrapping_add(*v))
                )
            );
        }
        if step % 4 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&terminal).unwrap(), &saved);
        } else {
            t.commit().unwrap();
            rows = candidate;
        }
        assert_eq!(g.source_rows(&source).unwrap(), &rows);
        if step % 75 == 0 {
            g.assert_consistent().unwrap();
        }
    }
    let saved = g.constraint_matches(&terminal).unwrap().clone();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(100, (99, 7))).unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &saved);
    g.assert_consistent().unwrap();
}
#[test]
fn nested_maps_keep_zero_groups_and_isolate_snapshots_and_clones() {
    type Nested = MapCollector<&'static str, (i32, i64), MapCollector<i32, i64, IntCollector>>;
    let mut c = Nested::default();
    assert!(!c.remove(&("missing", (0, 0))));
    c.insert(&("a", (1, 5)));
    c.insert(&("a", (1, -5)));
    c.insert(&("a", (2, 0)));
    c.insert(&("b", (1, 9)));
    let old = c.finish();
    let mut fork = c.clone();
    assert_eq!(old[&"a"][&1].sum, 0);
    assert_eq!(old[&"a"].len(), 2);
    assert!(!c.remove(&("a", (1, 99))));
    assert_eq!(c.finish(), old);
    assert!(c.remove(&("a", (1, 5))));
    assert_eq!(c.finish()[&"a"][&1].sum, -5);
    assert!(c.remove(&("a", (1, -5))));
    assert!(!c.finish()[&"a"].contains_key(&1));
    assert!(c.remove(&("a", (2, 0))));
    assert!(!c.finish().contains_key(&"a"));
    fork.insert(&("c", (3, i64::MAX)));
    fork.insert(&("c", (3, 1)));
    assert_eq!(fork.finish()[&"c"][&3].sum, i64::MIN);
    assert_eq!(old[&"a"][&1].count, 2);
    assert!(!old.contains_key(&"c"));
    assert_eq!(c.finish().keys().copied().collect::<Vec<_>>(), vec!["b"]);
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Nested>();
}
thread_local! { static FAILURE:Cell<u8> = const {Cell::new(0)}; }
fn fail_at(stage: u8) {
    FAILURE.with(|f| assert_ne!(f.get(), stage, "collector failure"));
}
#[derive(Default)]
struct Failing(IntCollector);
impl Clone for Failing {
    fn clone(&self) -> Self {
        fail_at(1);
        Self(self.0.clone())
    }
}
impl ReversibleCollector<i64> for Failing {
    type Output = i64;
    fn insert(&mut self, v: &i64) {
        self.0.insert(*v);
        fail_at(2);
    }
    fn remove(&mut self, v: &i64) -> bool {
        let result = self.0.remove(*v);
        fail_at(3);
        result
    }
    fn finish(&self) -> i64 {
        fail_at(4);
        self.0.summary().sum
    }
}
#[test]
fn inner_collector_panics_leave_both_maps_and_cached_outputs_unchanged() {
    let mut c = MapCollector::<i32, i64, Failing>::default();
    c.insert(&(1, 2));
    c.insert(&(1, 3));
    let old = c.finish();
    for stage in [1, 2, 4] {
        FAILURE.with(|f| f.set(stage));
        assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&(1, 7)))).is_err());
        FAILURE.with(|f| f.set(0));
        assert_eq!(c.finish(), old);
    }
    for stage in [1, 3, 4] {
        FAILURE.with(|f| f.set(stage));
        assert!(catch_unwind(AssertUnwindSafe(|| c.remove(&(1, 2)))).is_err());
        FAILURE.with(|f| f.set(0));
        assert_eq!(c.finish(), old);
    }
    assert!(!c.remove(&(1, 99)));
    assert_eq!(c.finish(), old);
    assert!(c.remove(&(1, 2)));
    assert_eq!(c.finish()[&1], 3);
    assert!(c.remove(&(1, 3)));
    assert!(c.finish().is_empty());
    assert_eq!(old[&1], 5);
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
fn trait_panics_cannot_split_membership_from_finished_map() {
    let budget = Rc::new(Cell::new(None));
    let key = |n| Key(n, budget.clone());
    let mut c = MapCollector::<Key, Key, SetCollector<Key>>::default();
    for (a, b) in [(1, 3), (2, 4), (2, 5)] {
        c.insert(&(key(a), key(b)));
    }
    let original = c.finish();
    let mut failures = 0;
    for operation in 0..3 {
        let mut successes = 0;
        for allowance in 0..100 {
            let mut trial = c.clone();
            budget.set(Some(allowance));
            let result = catch_unwind(AssertUnwindSafe(|| match operation {
                0 => trial.insert(&(key(2), key(6))),
                1 => {
                    assert!(trial.remove(&(key(2), key(4))));
                }
                _ => {
                    assert!(trial.remove(&(key(1), key(3))));
                }
            }));
            budget.set(None);
            if result.is_err() {
                failures += 1;
                assert_eq!(trial.finish(), original);
            } else {
                successes += 1;
                let m = trial.finish();
                match operation {
                    0 => assert_eq!(
                        m[&key(2)].keys().map(|k| k.0).collect::<Vec<_>>(),
                        vec![4, 5, 6]
                    ),
                    1 => assert_eq!(m[&key(2)].keys().map(|k| k.0).collect::<Vec<_>>(), vec![5]),
                    _ => assert!(!m.contains_key(&key(1))),
                }
            }
            assert_eq!(c.finish(), original);
        }
        assert!(successes > 0);
    }
    assert!(failures > 20);
}
#[test]
fn map_collector_scores_drive_construction_and_search() {
    let graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let counts = b
            .collect::<_, _, _, MapCollector<usize, (), CountCollector>>(&assigned, |(_, v)| {
                (v.unwrap(), ())
            })
            .unwrap();
        b.score("squared occupancy", &counts, |m| {
            SimpleScore::new(-m.values().map(|n| (*n as i64).pow(2)).sum::<i64>())
        })
        .unwrap();
        (b.build(), source)
    };
    let problem =
        FiniteProblem::new((0..6).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = graph();
    let constructed = greedy_stream_construction(
        &problem,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(constructed.complete);
    assert_eq!(constructed.score, Some(SimpleScore::new(-12)));
    let problem = FiniteProblem::new(
        (0..6)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let (g, s) = graph();
    let best = stream_local_search(
        &problem,
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
    let mut counts = [0i64; 3];
    for v in best.assignment.values() {
        counts[v.unwrap()] += 1;
    }
    assert_eq!(counts, [2, 2, 2]);
    assert_eq!(
        best.score,
        SimpleScore::new(-counts.iter().map(|n| n * n).sum::<i64>())
    );
    // Six items in three bins have sum of squares >= 12, attained here.
    assert_eq!(best.score, SimpleScore::new(-12));
}

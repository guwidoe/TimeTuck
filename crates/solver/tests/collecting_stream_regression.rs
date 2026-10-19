//! Original collector/graph regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Collector = PairCollector<IntCollector, SetCollector<i64>>;
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
fn reference(values: impl Iterator<Item = i64>) -> (IntSummary, Vec<i64>) {
    let v = values.collect::<Vec<_>>();
    let set = v.iter().copied().collect::<BTreeSet<_>>();
    (
        IntSummary {
            count: v.len(),
            distinct_count: set.len(),
            sum: v.iter().fold(0i64, |sum, v| sum.wrapping_add(*v)),
            min: v.iter().copied().min(),
            max: v.iter().copied().max(),
        },
        set.into_iter().collect(),
    )
}
#[test]
fn generated_key_moves_match_independent_statistics_and_sets() {
    let mut node = CollectNode::<usize, u32, (i64, i64), Collector>::new();
    let mut rows = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut seed = 20261022;
    for step in 0..5000 {
        let id = (random(&mut seed) % 40) as usize;
        let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
            (
                (random(&mut seed) % 5) as u32,
                match random(&mut seed) % 20 {
                    0 => i64::MAX,
                    1 => i64::MIN,
                    _ => (random(&mut seed) % 13) as i64 - 6,
                },
            )
        });
        if let Some(v) = value {
            rows.insert(id, v);
        } else {
            rows.remove(&id);
        }
        for change in node.apply(TupleChange {
            id,
            value: value.map(|(k, v)| (k, (v, v))),
        }) {
            if let Some((stats, set)) = change.value {
                outputs.insert(change.id, (stats, set.keys().copied().collect::<Vec<_>>()));
            } else {
                outputs.remove(&change.id);
            }
        }
        let expected = (0..5)
            .filter_map(|k| {
                let result = reference(rows.values().filter(|(g, _)| *g == k).map(|(_, v)| *v));
                (result.0.count > 0).then_some((k, result))
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(outputs, expected, "step {step}");
        assert_eq!(
            node.groups()
                .map(|(k, (s, set))| (*k, (s, set.keys().copied().collect::<Vec<_>>())))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
#[test]
fn empty_collector_values_do_not_create_global_stream_matches() {
    assert_eq!(IntCollector::default().summary(), IntSummary::default());
    let set = SetCollector::<i64>::default();
    assert!(set.finish().is_empty());
    assert_eq!(
        <CountCollector as ReversibleCollector<()>>::finish(&CountCollector::default()),
        0
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let counts = b
        .collect::<_, _, _, CountCollector>(source.port(), |_| ())
        .unwrap();
    let sums = b
        .collect::<_, _, _, IntCollector>(source.port(), |v| *v)
        .unwrap();
    let a = b
        .score("count presence", &counts, |_| SimpleScore::new(-1))
        .unwrap();
    let c = b
        .score("sum presence", &sums, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert!(g.constraint_matches(&a).unwrap().is_empty());
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    g.assert_consistent().unwrap();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, 0)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-2));
    assert_eq!(g.constraint_matches(&a).unwrap()[&()].0, 1);
    assert_eq!(g.constraint_matches(&c).unwrap()[&()].0.sum, 0);
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::retract(0)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::ZERO);
        t.assert_consistent().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(-2));
    let mut t = g.transaction();
    t.apply(&source, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert!(g.constraint_matches(&a).unwrap().is_empty());
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    g.assert_consistent().unwrap();
}
#[test]
fn nested_collectors_capture_independent_projections_and_snapshots() {
    type Nested = PairCollector<CountCollector, PairCollector<IntCollector, SetCollector<String>>>;
    let mut node = CollectNode::<usize, (), ((), (i64, String)), Nested>::new();
    node.apply(TupleChange::put(0, ((), ((), (5, "A".into())))));
    node.apply(TupleChange::put(1, ((), ((), (-2, "A".into())))));
    let (_, (count, (stats, snapshot))) = node.groups().next().unwrap();
    assert_eq!(count, 2);
    assert_eq!(stats.sum, 3);
    assert_eq!(snapshot.len(), 1);
    node.apply(TupleChange::put(0, ((), ((), (9, "B".into())))));
    let (_, (count, (stats, set))) = node.groups().next().unwrap();
    assert_eq!(count, 2);
    assert_eq!(stats.sum, 7);
    assert_eq!(set.keys().cloned().collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(snapshot.keys().cloned().collect::<Vec<_>>(), ["A"]);
    node.apply(TupleChange::retract(1));
    let (_, (count, (stats, set))) = node.groups().next().unwrap();
    assert_eq!(count, 1);
    assert_eq!(stats.sum, 9);
    assert_eq!(set.keys().cloned().collect::<Vec<_>>(), ["B"]);
}
#[derive(Clone, Default)]
struct FallibleCollector(Vec<i64>);
impl ReversibleCollector<i64> for FallibleCollector {
    type Output = Vec<i64>;
    fn insert(&mut self, v: &i64) {
        self.0.push(*v);
        assert_ne!(*v, 13, "insert panic");
    }
    fn remove(&mut self, v: &i64) -> bool {
        if *v == 17 {
            return false;
        }
        if let Some(i) = self.0.iter().position(|x| x == v) {
            self.0.remove(i);
            true
        } else {
            false
        }
    }
    fn finish(&self) -> Vec<i64> {
        assert!(!self.0.contains(&19), "finish panic");
        self.0.clone()
    }
}
#[test]
fn custom_collector_failures_preserve_prior_captured_values() {
    let mut n = CollectNode::<usize, (), i64, FallibleCollector>::new();
    n.apply(TupleChange::put(0, ((), 3)));
    for value in [13, 19] {
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply(TupleChange::put(0, ((), value)))
            ))
            .is_err()
        );
        assert_eq!(n.groups().next().unwrap().1, vec![3]);
    }
    n.apply(TupleChange::put(1, ((), 17)));
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply(TupleChange::retract(1)))).is_err());
    assert_eq!(n.groups().next().unwrap().1, vec![3, 17]);
    n.apply(TupleChange::retract(0));
    assert_eq!(n.groups().next().unwrap().1, vec![17]);
}
#[derive(Debug)]
struct Fallible(i32, Rc<Cell<Option<usize>>>);
impl Fallible {
    fn tick(&self) {
        if let Some(n) = self.1.get() {
            assert!(n > 0, "injected clone/order panic");
            self.1.set(Some(n - 1));
        }
    }
}
impl Clone for Fallible {
    fn clone(&self) -> Self {
        self.tick();
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Fallible {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b).is_eq()
    }
}
impl Eq for Fallible {}
impl PartialOrd for Fallible {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Fallible {
    fn cmp(&self, b: &Self) -> std::cmp::Ordering {
        self.tick();
        self.0.cmp(&b.0)
    }
}
#[test]
fn clone_ordering_panics_leave_all_group_indexes_unchanged() {
    let mut failed = 0;
    let mut succeeded = 0;
    for budget in 0..180 {
        let counter = Rc::new(Cell::new(None));
        let mut n = CollectNode::<usize, Fallible, Fallible, SetCollector<Fallible>>::new();
        for id in 0..12 {
            n.apply(TupleChange::put(
                id,
                (
                    Fallible((id % 3) as i32, counter.clone()),
                    Fallible(id as i32, counter.clone()),
                ),
            ));
        }
        counter.set(Some(budget));
        let r = catch_unwind(AssertUnwindSafe(|| {
            n.apply(TupleChange::put(
                0,
                (Fallible(1, counter.clone()), Fallible(20, counter.clone())),
            ))
        }));
        counter.set(None);
        let mut reference = (0..12)
            .map(|i| (i, (i % 3, i as i32)))
            .collect::<BTreeMap<_, _>>();
        if r.is_ok() {
            succeeded += 1;
            reference.insert(0, (1, 20));
        } else {
            failed += 1;
        }
        for id in 0..12 {
            let expected = (0..3)
                .filter_map(|k| {
                    let values = reference
                        .values()
                        .filter(|(g, _)| *g == k)
                        .map(|(_, v)| *v)
                        .collect::<BTreeSet<_>>();
                    (!values.is_empty()).then_some((k as i32, values))
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                n.groups()
                    .map(|(k, set)| (k.0, set.keys().map(|v| v.0).collect::<BTreeSet<_>>()))
                    .collect::<BTreeMap<_, _>>(),
                expected
            );
            n.apply(TupleChange::retract(id));
            reference.remove(&id);
        }
        assert_eq!(n.groups().count(), 0);
    }
    assert!(failed > 20 && succeeded > 0);
}
#[test]
fn keyed_graph_transactions_restore_composed_summaries_after_errors() {
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (u32, i64)>();
    let collected = b
        .group_collect_with::<_, _, _, _, Collector>(source.port(), |&(k, v)| (k, (v, v)))
        .unwrap();
    let terminal = b
        .score("groups", &collected, |(_, (s, _))| {
            SimpleScore::new(s.sum).with_structural(if fail.get() { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261023;
    for step in 0..1000 {
        let mut candidate = rows.clone();
        let mut tx = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 20) as usize;
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
            tx.apply(&source, TupleChange { id, value }).unwrap();
        }
        let expect = (0..4)
            .filter_map(|k| {
                let r = reference(candidate.values().filter(|(g, _)| *g == k).map(|(_, v)| *v));
                (r.0.count > 0).then_some((k, r))
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            tx.graph()
                .unwrap()
                .constraint_matches(&terminal)
                .unwrap()
                .iter()
                .map(|(&k, ((_, (s, set)), _))| (k, (*s, set.keys().copied().collect::<Vec<_>>())))
                .collect::<BTreeMap<_, _>>(),
            expect
        );
        if step % 4 == 0 {
            drop(tx);
        } else {
            tx.commit().unwrap();
            rows = candidate;
        }
        assert_eq!(g.source_rows(&source).unwrap(), &rows);
        if step % 50 == 0 {
            g.assert_consistent().unwrap();
        }
    }
    let old = g.score();
    fail.set(true);
    {
        let mut t = g.transaction();
        assert!(t.apply(&source, TupleChange::put(99, (99, 4))).is_err());
        assert!(t.commit().is_err());
    }
    fail.set(false);
    assert_eq!(g.score(), old);
    g.assert_consistent().unwrap();
}
#[test]
fn global_collector_score_integrates_with_construction_and_search() {
    let graph = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let sum = b
            .collect::<_, _, _, IntCollector>(&assigned, |(_, v)| v.unwrap() as i64)
            .unwrap();
        b.score("sum", &sum, |s| SimpleScore::new(-s.sum)).unwrap();
        (b.build(), source)
    };
    let p = FiniteProblem::new((0..3).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
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
        (0..3)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(2))
            .collect(),
    )
    .unwrap();
    let (g, s) = graph();
    let r = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            assert_scores: true,
            max_evaluations: 100,
            max_steps: 10,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(r.score, SimpleScore::ZERO);
    assert_eq!(r.assignment.values(), &[Some(0), Some(0), Some(0)]);
}

// Original replacement staging failure/persistence checks.
mod set_replace_staging {
    use std::{
        cell::RefCell,
        cmp::Ordering,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    use timetuck::{ReversibleCollector, SetCollector};

    #[derive(Default)]
    struct Control {
        events: Vec<&'static str>,
        fail_at: Option<usize>,
    }
    struct Key(i32, Rc<RefCell<Control>>);
    impl Key {
        fn event(&self, event: &'static str) {
            let mut control = self.1.borrow_mut();
            control.events.push(event);
            assert_ne!(
                control.fail_at,
                Some(control.events.len()),
                "injected callback failure"
            );
        }
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.event("clone");
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            self.event("compare");
            self.0.cmp(&other.0)
        }
    }
    fn baseline(collector: &mut SetCollector<Key>, old: &Key, new: &Key) -> bool {
        let mut next = collector.clone();
        if !next.remove(old) {
            return false;
        }
        next.insert(new);
        *collector = next;
        true
    }
    fn fixture(control: &Rc<RefCell<Control>>) -> SetCollector<Key> {
        let mut collector = SetCollector::default();
        for value in (0..64).chain([32]) {
            collector.insert(&Key(value, control.clone()));
        }
        control.borrow_mut().events.clear();
        collector
    }

    #[test]
    fn replacement_retains_callback_sequence_and_rolls_back_each_callback_panic() {
        // Unique and duplicate old values; existing/new/equal destinations; absent old.
        for (old, new) in [
            (31, 100),
            (32, 100),
            (31, 33),
            (32, 33),
            (31, 31),
            (32, 32),
            (99, 100),
        ] {
            let control = Rc::new(RefCell::new(Control::default()));
            let mut expected = fixture(&control);
            let old_key = Key(old, control.clone());
            let new_key = Key(new, control.clone());
            let found = baseline(&mut expected, &old_key, &new_key);
            let events = control.borrow().events.clone();
            let expected_values = expected
                .finish()
                .keys()
                .map(|key| key.0)
                .collect::<Vec<_>>();
            let mut actual = fixture(&control);
            let saved = actual.finish();
            assert_eq!(actual.replace(&old_key, &new_key), found);
            assert_eq!(control.borrow().events, events);
            assert_eq!(
                actual.finish().keys().map(|key| key.0).collect::<Vec<_>>(),
                expected_values
            );
            assert_eq!(
                saved.keys().map(|key| key.0).collect::<Vec<_>>(),
                (0..64).collect::<Vec<_>>()
            );
            // Drain all occurrences, checking multiplicities beyond distinct output.
            for value in 0..=100 {
                let key = Key(value, control.clone());
                loop {
                    let expected_removed = expected.remove(&key);
                    assert_eq!(actual.remove(&key), expected_removed);
                    if !expected_removed {
                        break;
                    }
                }
            }
            assert!(actual.finish().is_empty());
            for fail_at in 1..=events.len() {
                let mut actual = fixture(&control);
                let saved = actual.finish();
                control.borrow_mut().fail_at = Some(fail_at);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| actual.replace(&old_key, &new_key))).is_err()
                );
                control.borrow_mut().fail_at = None;
                assert_eq!(
                    actual.finish().keys().map(|key| key.0).collect::<Vec<_>>(),
                    (0..64).collect::<Vec<_>>()
                );
                assert_eq!(saved.len(), 64);
                for value in (0..64).chain([32]) {
                    assert!(actual.remove(&Key(value, control.clone())));
                }
                assert!(actual.finish().is_empty());
            }
        }
    }
}

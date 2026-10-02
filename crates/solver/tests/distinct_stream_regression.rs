//! Original regressions; no upstream-method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value {
    key: i32,
    payload: i64,
}
impl PartialEq for Value {
    fn eq(&self, b: &Self) -> bool {
        self.key == b.key
    }
}
impl Eq for Value {}
impl PartialOrd for Value {
    fn partial_cmp(&self, b: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Value {
    fn cmp(&self, b: &Self) -> std::cmp::Ordering {
        self.key.cmp(&b.key)
    }
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
fn expected(rows: &BTreeMap<usize, Value>) -> BTreeMap<i32, i64> {
    let mut out = BTreeMap::new();
    for row in rows.values() {
        out.entry(row.key).or_insert(row.payload);
    }
    out
}
#[test]
fn generated_edits_preserve_lowest_id_payloads_and_occurrence_counts() {
    let mut n = DistinctNode::new();
    let mut inputs = BTreeMap::new();
    let mut emitted = BTreeMap::new();
    let mut seed = 20261020;
    for step in 0..6000 {
        let id = (random(&mut seed) % 96) as usize;
        let value = (!random(&mut seed).is_multiple_of(4)).then(|| Value {
            key: (random(&mut seed) % 13) as i32,
            payload: (random(&mut seed) % 41) as i64 - 20,
        });
        if let Some(v) = &value {
            inputs.insert(id, v.clone());
        } else {
            inputs.remove(&id);
        }
        for change in n.apply(TupleChange { id, value }) {
            if let Some(v) = change.value {
                emitted.insert(change.id.key, v.payload);
            } else {
                emitted.remove(&change.id.key);
            }
        }
        assert_eq!(emitted, expected(&inputs), "step {step}");
        assert_eq!(
            n.rows()
                .map(|(k, v)| (k.key, v.payload))
                .collect::<BTreeMap<_, _>>(),
            emitted
        );
        for key in 0..13 {
            assert_eq!(
                n.occurrences(&Value { key, payload: 0 }),
                inputs.values().filter(|v| v.key == key).count()
            );
        }
    }
}
#[test]
fn representative_updates_do_not_leave_stale_equal_payloads() {
    let mut n = DistinctNode::new();
    let put = |id, payload| TupleChange::put(id, Value { key: 1, payload });
    assert_eq!(n.apply(put(5, 50))[0].value.as_ref().unwrap().payload, 50);
    assert!(n.apply(put(8, 80)).is_empty());
    assert_eq!(n.apply(put(2, 20))[0].value.as_ref().unwrap().payload, 20);
    assert_eq!(n.apply(put(2, 21))[0].value.as_ref().unwrap().payload, 21);
    assert!(n.apply(put(8, 81)).is_empty());
    assert_eq!(
        n.apply(TupleChange::retract(2))[0]
            .value
            .as_ref()
            .unwrap()
            .payload,
        50
    );
    assert!(n.apply(TupleChange::retract(8)).is_empty());
    assert!(n.apply(TupleChange::retract(5))[0].value.is_none());
    assert!(n.apply(TupleChange::retract(5)).is_empty());
}
#[test]
fn nullable_tuple_values_are_distinct_from_retraction() {
    let mut n = DistinctNode::<usize, Option<(u32, u32)>>::new();
    assert_eq!(
        n.apply(TupleChange::put(0, None)),
        vec![TupleChange::put(None, None)]
    );
    assert!(n.apply(TupleChange::put(1, None)).is_empty());
    assert_eq!(
        n.apply(TupleChange::put(2, Some((1, 2)))),
        vec![TupleChange::put(Some((1, 2)), Some((1, 2)))]
    );
    n.apply(TupleChange::retract(0));
    assert_eq!(n.occurrences(&None), 1);
    assert_eq!(
        n.apply(TupleChange::retract(1)),
        vec![TupleChange::retract(None)]
    );
    assert_eq!(
        n.rows().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
        vec![(Some((1, 2)), Some((1, 2)))]
    );
}
#[test]
fn composed_distinct_transactions_restore_representatives_and_snapshots() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Value>();
    let once = b.distinct(source.port()).unwrap();
    let twice = b.distinct(&once).unwrap();
    let terminal = b
        .score("distinct", &twice, |v| SimpleScore::new(v.payload))
        .unwrap();
    let mut g = b.build();
    let mut inputs = BTreeMap::new();
    let mut seed = 20261021;
    for step in 0..1000 {
        let snapshot = g.constraint_matches(&terminal).unwrap().clone();
        let original = expected(&inputs);
        let mut candidate = inputs.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let id = (random(&mut seed) % 20) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| Value {
                key: (random(&mut seed) % 5) as i32,
                payload: (random(&mut seed) % 31) as i64 - 15,
            });
            if let Some(v) = &value {
                candidate.insert(id, v.clone());
            } else {
                candidate.remove(&id);
            }
            t.apply(&source, TupleChange { id, value }).unwrap();
            let reference = expected(&candidate);
            let graph = t.graph().unwrap();
            assert_eq!(
                graph
                    .constraint_matches(&terminal)
                    .unwrap()
                    .values()
                    .map(|(v, s)| {
                        assert_eq!(*s, SimpleScore::new(v.payload));
                        (v.key, v.payload)
                    })
                    .collect::<BTreeMap<_, _>>(),
                reference
            );
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(reference.values().sum())
            );
        }
        if step % 3 == 0 {
            drop(t);
        } else {
            t.commit().unwrap();
            inputs = candidate;
        }
        assert_eq!(
            snapshot
                .values()
                .map(|(v, _)| (v.key, v.payload))
                .collect::<BTreeMap<_, _>>(),
            original
        );
        assert_eq!(
            g.constraint_matches(&terminal)
                .unwrap()
                .values()
                .map(|(v, _)| (v.key, v.payload))
                .collect::<BTreeMap<_, _>>(),
            expected(&inputs)
        );
        if step % 40 == 0 {
            g.assert_consistent().unwrap();
        }
    }
}
#[derive(Debug)]
struct Fallible(i32, Rc<Cell<Option<usize>>>);
impl Fallible {
    fn tick(&self) {
        if let Some(n) = self.1.get() {
            assert!(n > 0, "injected failure");
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
fn clone_and_ordering_panics_do_not_install_partial_state() {
    let mut failures = 0;
    let mut successes = 0;
    for budget in 0..180 {
        let counter = Rc::new(Cell::new(None));
        let mut n = DistinctNode::new();
        for id in 0..20 {
            n.apply(TupleChange::put(id, Fallible(id % 7, counter.clone())));
        }
        counter.set(Some(budget));
        let result = catch_unwind(AssertUnwindSafe(|| {
            n.apply(TupleChange::put(3, Fallible(9, counter.clone())))
        }));
        counter.set(None);
        let mut reference = (0..20).map(|i| (i, i % 7)).collect::<BTreeMap<_, _>>();
        if result.is_ok() {
            successes += 1;
            reference.insert(3, 9);
        } else {
            failures += 1;
        }
        for id in 0..20 {
            assert_eq!(
                n.rows().map(|(k, _)| k.0).collect::<BTreeSet<_>>(),
                reference.values().copied().collect()
            );
            for key in 0..10 {
                assert_eq!(
                    n.occurrences(&Fallible(key, counter.clone())),
                    reference.values().filter(|&&v| v == key).count()
                );
            }
            n.apply(TupleChange::retract(id));
            reference.remove(&id);
        }
        assert_eq!(n.rows().count(), 0);
    }
    assert!(failures > 20 && successes > 0);
}
#[test]
fn representative_changes_roll_back_after_score_errors_and_panics() {
    let fault = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Value>();
    let port = b.distinct(source.port()).unwrap();
    let terminal = b
        .score("value", &port, |v| {
            assert_ne!(fault.get(), 2, "score panic");
            SimpleScore::new(v.payload).with_structural(if fault.get() == 1 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, payload) in [(0, 3), (1, 7)] {
        t.apply(&source, TupleChange::put(id, Value { key: 0, payload }))
            .unwrap();
    }
    t.commit().unwrap();
    for kind in [1, 2] {
        fault.set(kind);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let r = t.apply(&source, TupleChange::retract(0));
            assert!(r.is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(result.is_err(), kind == 2);
        fault.set(0);
        assert_eq!(g.score(), SimpleScore::new(3));
        assert_eq!(
            g.constraint_matches(&terminal)
                .unwrap()
                .values()
                .next()
                .unwrap()
                .0
                .payload,
            3
        );
        g.assert_consistent().unwrap();
    }
}
#[test]
fn construction_and_search_minimize_distinct_assigned_values() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let values = b.map(&assigned, |(_, v)| v.unwrap()).unwrap();
        let distinct = b.distinct(&values).unwrap();
        b.score("machines", &distinct, |_| SimpleScore::new(-1))
            .unwrap();
        (b.build(), source)
    };
    let p = FiniteProblem::new((0..3).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = build();
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
    assert_eq!(c.score, Some(SimpleScore::new(-1)));
    let p = FiniteProblem::new(
        (0..3)
            .map(|i| Variable::new(vec![0, 1, 2]).with_initial(i))
            .collect(),
    )
    .unwrap();
    let (g, s) = build();
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
    assert_eq!(r.score, SimpleScore::new(-1));
    assert!(p.is_complete(&r.assignment));
    assert_eq!(
        r.assignment.values().iter().collect::<BTreeSet<_>>().len(),
        1
    );
}

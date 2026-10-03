//! Original regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
const RELATIONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
type Row = (i32, i32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
fn accepts(c: JoinComparison, a: &Row, b: &Row) -> bool {
    a.0 == b.0
        && match c {
            JoinComparison::Equal => a.1 == b.1,
            JoinComparison::LessThan => a.1 < b.1,
            JoinComparison::LessThanOrEqual => a.1 <= b.1,
            JoinComparison::GreaterThan => a.1 > b.1,
            JoinComparison::GreaterThanOrEqual => a.1 >= b.1,
        }
}
fn pairs(c: JoinComparison, l: &BTreeMap<usize, Row>, r: &BTreeMap<usize, Row>) -> Pairs {
    l.iter()
        .flat_map(|(&i, &a)| {
            r.iter()
                .filter(move |(_, b)| accepts(c, &a, b))
                .map(move |(&j, &b)| ((i, j), (a, b)))
        })
        .collect()
}
fn apply<K: Ord, V>(
    rows: &mut BTreeMap<K, V>,
    changes: impl IntoIterator<Item = TupleChange<K, V>>,
) {
    for c in changes {
        if let Some(v) = c.value {
            rows.insert(c.id, v);
        } else {
            rows.remove(&c.id);
        }
    }
}
fn random(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    *s >> 32
}
#[test]
fn changing_both_keys_and_payloads_matches_full_pairs_in_both_directions() {
    let mut seed = 20261114;
    for c in RELATIONS {
        let calls = Cell::new(0);
        let project = |r: &Row| {
            calls.set(calls.get() + 1);
            (r.0, r.1)
        };
        let mut j = PartitionedJoinNode::new(c, project, project);
        let mut l = BTreeMap::new();
        let mut r = BTreeMap::new();
        let mut out = Pairs::new();
        for step in 0..1200 {
            let id = (random(&mut seed) % 24) as usize;
            let value = (!random(&mut seed).is_multiple_of(5)).then(|| {
                (
                    (random(&mut seed) % 4) as i32,
                    (random(&mut seed) % 7) as i32 - 3,
                    step as i64,
                )
            });
            let change = TupleChange { id, value };
            calls.set(0);
            let events = if random(&mut seed).is_multiple_of(2) {
                apply(&mut l, [change.clone()]);
                j.apply_left(change)
            } else {
                apply(&mut r, [change.clone()]);
                j.apply_right(change)
            };
            assert_eq!(calls.get(), usize::from(value.is_some()));
            apply(&mut out, events);
            let expected = pairs(c, &l, &r);
            assert_eq!(out, expected, "{c:?} step{step}");
            assert_eq!(
                j.matches()
                    .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                    .collect::<Pairs>(),
                expected
            );
        }
    }
}
#[test]
fn shared_source_transactions_restore_partitions_ranges_and_exact_matches() {
    let mut seed = 20261115;
    for c in RELATIONS {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Row>();
        let join = b
            .join_partitioned(
                source.port(),
                source.port(),
                c,
                |r| (r.0, r.1),
                |r| (r.0, r.1),
            )
            .unwrap();
        let terminal = b
            .score("pairs", &join, |(a, b)| SimpleScore::new(a.2 - b.2))
            .unwrap();
        let mut g = b.build();
        let mut rows = BTreeMap::new();
        for step in 0..600 {
            let before = g.constraint_matches(&terminal).unwrap().clone();
            let mut next = rows.clone();
            let mut t = g.transaction();
            for _ in 0..1 + random(&mut seed) % 3 {
                let id = (random(&mut seed) % 12) as usize;
                let value = (!random(&mut seed).is_multiple_of(5)).then(|| {
                    (
                        (random(&mut seed) % 3) as i32,
                        (random(&mut seed) % 5) as i32,
                        (random(&mut seed) % 31) as i64 - 15,
                    )
                });
                let change = TupleChange { id, value };
                apply(&mut next, [change.clone()]);
                t.apply(&source, change).unwrap();
                let expected = pairs(c, &next, &next)
                    .into_iter()
                    .map(|(id, (a, b))| (id, ((a, b), SimpleScore::new(a.2 - b.2))))
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(
                    t.graph().unwrap().constraint_matches(&terminal).unwrap(),
                    &expected
                );
                assert_eq!(
                    t.score().unwrap(),
                    SimpleScore::new(expected.values().map(|((a, b), _)| a.2 - b.2).sum())
                );
            }
            if step % 3 == 0 {
                drop(t);
                assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
            } else {
                t.commit().unwrap();
                rows = next;
            }
            if step % 37 == 0 {
                g.assert_consistent().unwrap();
            }
        }
        g.assert_consistent().unwrap();
    }
}
#[derive(Clone, Debug)]
struct Counted(i32, Rc<Cell<usize>>);
impl PartialEq for Counted {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
    }
}
impl Eq for Counted {}
impl PartialOrd for Counted {
    fn partial_cmp(&self, r: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Counted {
    fn cmp(&self, r: &Self) -> std::cmp::Ordering {
        self.1.set(self.1.get() + 1);
        self.0.cmp(&r.0)
    }
}
#[test]
fn sparse_queries_skip_unrelated_partitions_and_nonmatching_ordered_keys() {
    for (c, q, wanted) in [
        (JoinComparison::Equal, 2048, 2048),
        (JoinComparison::LessThan, 4094, 4095),
        (JoinComparison::LessThanOrEqual, 4095, 4095),
        (JoinComparison::GreaterThan, 1, 0),
        (JoinComparison::GreaterThanOrEqual, 0, 0),
    ] {
        for reversed in [false, true] {
            let count = Rc::new(Cell::new(0));
            let project =
                |r: &(i32, i32)| (Counted(r.0, count.clone()), Counted(r.1, count.clone()));
            let mut j =
                PartitionedJoinNode::new(if reversed { c.reversed() } else { c }, project, project);
            for id in 0..8192 {
                let row = if id < 4096 { (0, id) } else { (id - 4095, 0) };
                let change = TupleChange::put(id, row);
                if reversed {
                    j.apply_left(change);
                } else {
                    j.apply_right(change);
                }
            }
            count.set(0);
            let change = TupleChange::put(-1, (0, q));
            let events = if reversed {
                j.apply_right(change)
            } else {
                j.apply_left(change)
            };
            assert!(count.get() < 512, "{c:?}: {} comparisons", count.get());
            assert_eq!(
                events,
                vec![if reversed {
                    TupleChange::put((wanted, -1), ((0, wanted), (0, q)))
                } else {
                    TupleChange::put((-1, wanted), ((0, q), (0, wanted)))
                }]
            );
        }
    }
}
#[derive(Debug)]
struct Fragile {
    value: i32,
    clones: Rc<Cell<usize>>,
    comparisons: Rc<Cell<usize>>,
}
fn tick(count: &Cell<usize>) {
    let n = count.get();
    if n > 0 {
        count.set(n - 1);
        assert_ne!(n, 1, "injected operation panic");
    }
}
impl Clone for Fragile {
    fn clone(&self) -> Self {
        tick(&self.clones);
        Self {
            value: self.value,
            clones: self.clones.clone(),
            comparisons: self.comparisons.clone(),
        }
    }
}
impl PartialEq for Fragile {
    fn eq(&self, r: &Self) -> bool {
        self.value == r.value
    }
}
impl Eq for Fragile {}
impl PartialOrd for Fragile {
    fn partial_cmp(&self, r: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Fragile {
    fn cmp(&self, r: &Self) -> std::cmp::Ordering {
        tick(&self.comparisons);
        self.value.cmp(&r.value)
    }
}
#[test]
fn every_clone_and_comparison_failure_preserves_installed_standalone_roots() {
    // Sweep each operation in a partition transfer, including failures after the
    // old partition was already removed from the persistent candidate.
    for right in [false, true] {
        for clone_fault in [false, true] {
            let mut failures = 0;
            let mut completed = false;
            for at in 1..80 {
                let clones = Rc::new(Cell::new(0));
                let comparisons = Rc::new(Cell::new(0));
                let key = |v| Fragile {
                    value: v,
                    clones: clones.clone(),
                    comparisons: comparisons.clone(),
                };
                let project = |r: &Row| (key(r.0), key(r.1));
                let mut j =
                    PartitionedJoinNode::new(JoinComparison::LessThanOrEqual, project, project);
                for i in 0..5 {
                    j.apply_left(TupleChange::put(i, (i as i32 % 2, i as i32, i as i64)));
                    j.apply_right(TupleChange::put(i, (i as i32 % 2, i as i32 + 1, i as i64)));
                }
                let before = j
                    .matches()
                    .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                    .collect::<Pairs>();
                if clone_fault {
                    clones.set(at);
                } else {
                    comparisons.set(at);
                }
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if right {
                        j.apply_right(TupleChange::put(0, (1, 3, 99)))
                    } else {
                        j.apply_left(TupleChange::put(0, (1, 3, 99)))
                    }
                }));
                clones.set(0);
                comparisons.set(0);
                if result.is_ok() {
                    completed = true;
                    break;
                }
                failures += 1;
                assert_eq!(
                    j.matches()
                        .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                        .collect::<Pairs>(),
                    before
                );
                // Retraction after failure checks the separate membership lookup too.
                if right {
                    j.apply_right(TupleChange::retract(0));
                } else {
                    j.apply_left(TupleChange::retract(0));
                }
                assert!(
                    j.matches()
                        .all(|((i, j), _)| if right { *j != 0 } else { *i != 0 })
                );
            }
            assert!(completed, "fault sweep must reach a successful update");
            assert!(
                failures >= 4,
                "exercise multiple phases of candidate preparation"
            );
        }
    }
}
#[test]
fn graph_errors_panics_foreign_ports_and_failed_rebuild_preserve_partition_state() {
    let fault = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, Row>();
    let right = b.source::<usize, Row>();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = other.source::<usize, Row>();
    for (l, r) in [
        (left.port(), foreign.port()),
        (foreign.port(), right.port()),
    ] {
        assert!(matches!(
            b.join_partitioned(
                l,
                r,
                JoinComparison::LessThan,
                |r| (r.0, r.1),
                |r| (r.0, r.1)
            ),
            Err(GraphError::ForeignPort)
        ));
    }
    let joined = b
        .join_partitioned(
            left.port(),
            right.port(),
            JoinComparison::LessThan,
            |r| {
                assert_ne!(fault.get(), 3);
                (r.0, r.1)
            },
            |r| {
                assert_ne!(fault.get(), 4);
                (r.0, r.1)
            },
        )
        .unwrap();
    let terminal = b
        .score("all", &joined, |(_, r)| {
            calls.set(calls.get() + 1);
            assert!(!(fault.get() == 1 && r.2 == 3));
            SimpleScore::new(r.2).with_structural(if fault.get() == 2 && r.2 == 3 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&left, TupleChange::put(0, (0, 0, 0))).unwrap();
    for (id, row) in [
        (0, (0, 1, 5)),
        (1, (1, 1, 1)),
        (2, (1, 2, 2)),
        (3, (1, 3, 3)),
    ] {
        t.apply(&right, TupleChange::put(id, row)).unwrap();
    }
    t.commit().unwrap();
    let before = g.constraint_matches(&terminal).unwrap().clone();
    for mode in 1..=4 {
        fault.set(mode);
        calls.set(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let r = if mode == 4 {
                t.apply(&right, TupleChange::put(0, (1, 4, 4)))
            } else {
                t.apply(&left, TupleChange::put(0, (1, 0, 0)))
            };
            assert!(r.is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(outcome.is_err(), mode != 2);
        if mode <= 2 {
            assert_eq!(calls.get(), 3);
        }
        fault.set(0);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
        g.assert_consistent().unwrap();
    }
    fault.set(3);
    assert!(catch_unwind(AssertUnwindSafe(|| g.assert_consistent())).is_err());
    fault.set(0);
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    let mut t = g.transaction();
    t.apply(&left, TupleChange::put(0, (1, 0, 0))).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(6));
    g.assert_consistent().unwrap();
}
#[test]
fn grouped_multi_equalities_and_range_changes_propagate_exact_rows_and_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, ((i32, i32), i64)>();
    let caps = b.source::<usize, ((i32, i32), i64)>();
    let groups = b.group_collect(source.port(), |r| *r).unwrap();
    let joined = b
        .join_partitioned(
            &groups,
            caps.port(),
            JoinComparison::GreaterThan,
            |(e, s)| (*e, s.sum),
            |r| *r,
        )
        .unwrap();
    let terminal = b
        .score("overload", &joined, |((_, s), (_, cap))| {
            SimpleScore::new(cap - s.sum)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, row) in [(0, ((0, 0), 4)), (1, ((0, 0), 3)), (2, ((0, 1), 12))] {
        t.apply(&source, TupleChange::put(id, row)).unwrap();
    }
    for (id, row) in [(0, ((0, 0), 5)), (1, ((0, 1), 10)), (2, ((1, 0), 0))] {
        t.apply(&caps, TupleChange::put(id, row)).unwrap();
    }
    t.commit().unwrap();
    let before = g.constraint_matches(&terminal).unwrap().clone();
    assert_eq!(g.score(), SimpleScore::new(-4));
    assert_eq!(
        before.keys().copied().collect::<Vec<_>>(),
        vec![((0, 0), 0), ((0, 1), 1)]
    );
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(1, ((0, 1), 4))).unwrap();
        let m = t.graph().unwrap().constraint_matches(&terminal).unwrap();
        assert_eq!(m.keys().copied().collect::<Vec<_>>(), vec![((0, 1), 1)]);
        assert_eq!(m[&((0, 1), 1)].0.0.1.sum, 16);
        assert_eq!(t.score().unwrap(), SimpleScore::new(-6));
        t.assert_consistent().unwrap();
    }
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    g.assert_consistent().unwrap();
}
#[test]
fn construction_and_local_search_order_values_within_each_machine() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(s.port(), |r| r.1.is_some()).unwrap();
        let j = b
            .join_partitioned(
                &assigned,
                &assigned,
                JoinComparison::GreaterThanOrEqual,
                |r| (r.0 / 3, r.1.unwrap()),
                |r| (r.0 / 3, r.1.unwrap()),
            )
            .unwrap();
        let p = b.filter(&j, |(a, b)| a.0 < b.0).unwrap();
        b.score("order", &p, |_| SimpleScore::new(-1)).unwrap();
        (b.build(), s)
    };
    let p = FiniteProblem::new((0..6).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
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
    assert_eq!(c.score, Some(SimpleScore::ZERO));
    assert_eq!(
        c.assignment.values(),
        &[Some(0), Some(1), Some(2), Some(0), Some(1), Some(2)]
    );
    let p = FiniteProblem::new(
        (0..6)
            .map(|i| Variable::new(vec![0, 1, 2]).with_initial(2 - i % 3))
            .collect(),
    )
    .unwrap();
    let (g, s) = build();
    let best = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            assert_scores: true,
            max_steps: 30,
            max_evaluations: 1000,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    let v = best.assignment.values();
    assert_eq!(v, &[Some(0), Some(1), Some(2), Some(0), Some(1), Some(2)]);
    let bad = (0..6)
        .flat_map(|i| (i + 1..6).map(move |j| (i, j)))
        .filter(|&(i, j)| i / 3 == j / 3 && v[i] >= v[j])
        .count();
    assert_eq!(best.score, SimpleScore::new(-(bad as i64)));
    assert_eq!(best.score, SimpleScore::ZERO);
}

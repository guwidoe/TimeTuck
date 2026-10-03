//! Original regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (i32, i32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
fn pairs(l: &BTreeMap<usize, Row>, r: &BTreeMap<usize, Row>) -> Pairs {
    l.iter()
        .flat_map(|(&i, &a)| {
            r.iter()
                .filter(move |(_, b)| a.0 == b.0 && i64::from(a.1) < b.2 && i64::from(b.1) < a.2)
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
    for _ in 0..3 {
        let calls = Cell::new(0);
        let project = |r: &Row| {
            calls.set(calls.get() + 1);
            (r.0, i64::from(r.1), r.2)
        };
        let mut j = OverlapJoinNode::partitioned(project, project);
        let mut l = BTreeMap::new();
        let mut r = BTreeMap::new();
        let mut out = Pairs::new();
        for step in 0..1200 {
            let id = (random(&mut seed) % 24) as usize;
            let value = (!random(&mut seed).is_multiple_of(5)).then(|| {
                (
                    (random(&mut seed) % 4) as i32,
                    (random(&mut seed) % 7) as i32 - 3,
                    (random(&mut seed) % 13) as i64 - 6,
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
            let expected = pairs(&l, &r);
            assert_eq!(out, expected, "step{step}");
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
    for _ in 0..3 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Row>();
        let join = b
            .join_overlapping_partitioned(
                source.port(),
                source.port(),
                |r| (r.0, i64::from(r.1), r.2),
                |r| (r.0, i64::from(r.1), r.2),
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
                let expected = pairs(&next, &next)
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
            for at in 1..200 {
                let clones = Rc::new(Cell::new(0));
                let comparisons = Rc::new(Cell::new(0));
                let key = |v| Fragile {
                    value: v,
                    clones: clones.clone(),
                    comparisons: comparisons.clone(),
                };
                let project = |r: &Row| (key(r.0), key(r.1), key(r.2 as i32));
                let mut j = OverlapJoinNode::partitioned(project, project);
                for i in 0..5 {
                    j.apply_left(TupleChange::put(i, (i as i32 % 2, i as i32, i as i64 + 3)));
                    j.apply_right(TupleChange::put(
                        i,
                        (i as i32 % 2, i as i32 + 1, i as i64 + 3),
                    ));
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
fn endpoint_and_payload_replacements_emit_exact_events() {
    let project = |r: &(i32, i32, &str)| (r.0, r.1);
    let mut j = OverlapJoinNode::new(project, project);
    j.apply_left(TupleChange::put(1, (1, 5, "a")));
    assert_eq!(
        j.apply_right(TupleChange::put(2, (2, 4, "b"))),
        vec![TupleChange::put((1, 2), ((1, 5, "a"), (2, 4, "b")))]
    );
    assert_eq!(
        j.apply_left(TupleChange::put(1, (1, 5, "c"))),
        vec![TupleChange::put((1, 2), ((1, 5, "c"), (2, 4, "b")))]
    );
    assert_eq!(
        j.apply_left(TupleChange::put(1, (3, 6, "d"))),
        vec![
            TupleChange::retract((1, 2)),
            TupleChange::put((1, 2), ((3, 6, "d"), (2, 4, "b")))
        ]
    );
    assert_eq!(
        j.apply_right(TupleChange::put(2, (6, 7, "e"))),
        vec![TupleChange::retract((1, 2))]
    );
    assert!(j.apply_right(TupleChange::retract(99)).is_empty());
}
#[test]
fn graph_downstream_failures_and_projection_panics_restore_all_interval_roots() {
    let fault = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (i32, i32)>();
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let f = foreign.source::<usize, (i32, i32)>();
    for (l, r) in [(s.port(), f.port()), (f.port(), s.port())] {
        assert!(matches!(
            b.join_overlapping(l, r, |r| *r, |r| *r),
            Err(GraphError::ForeignPort)
        ));
        assert!(matches!(
            b.join_overlapping_partitioned(l, r, |r| ((), r.0, r.1), |r| ((), r.0, r.1)),
            Err(GraphError::ForeignPort)
        ));
    }
    let join = b
        .join_overlapping(
            s.port(),
            s.port(),
            |r| {
                assert!(!fault.get());
                *r
            },
            |r| *r,
        )
        .unwrap();
    let terminal = b
        .score("pairs", &join, |(a, _)| {
            assert_ne!(a.0, 99);
            SimpleScore::new(1).with_structural(if a.0 == 98 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, (0, 200))).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&terminal).unwrap().clone();
    for (start, panic) in [(98, false), (99, true)] {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            assert!(t.apply(&s, TupleChange::put(1, (start, 100))).is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(outcome.is_err(), panic);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
        g.assert_consistent().unwrap();
    }
    fault.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let _ = t.apply(&s, TupleChange::put(0, (1, 4)));
        }))
        .is_err()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| g.assert_consistent())).is_err());
    fault.set(false);
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    g.assert_consistent().unwrap();
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
fn sparse_intervals_skip_unrelated_partitions_and_ended_subtrees() {
    for reverse in [false, true] {
        let count = Rc::new(Cell::new(0));
        let project = |r: &(i32, i32, i32)| {
            (
                Counted(r.0, count.clone()),
                Counted(r.1, count.clone()),
                Counted(r.2, count.clone()),
            )
        };
        let mut j = OverlapJoinNode::partitioned(project, project);
        for i in 0..8192 {
            let row = if i < 4096 {
                (0, 2 * i, 2 * i + 1)
            } else {
                (i, 0, 10000)
            };
            if reverse {
                j.apply_left(TupleChange::put(i, row));
            } else {
                j.apply_right(TupleChange::put(i, row));
            }
        }
        for (q, expected) in [(8190, Some(4095)), (9000, None), (-2, None)] {
            count.set(0);
            let events = if reverse {
                j.apply_right(TupleChange::put(-1, (0, q, q + 1)))
            } else {
                j.apply_left(TupleChange::put(-1, (0, q, q + 1)))
            };
            assert!(count.get() < 512, "{} comparisons", count.get());
            let puts = events
                .into_iter()
                .filter(|e| e.value.is_some())
                .map(|e| if reverse { e.id.0 } else { e.id.1 })
                .collect::<Vec<_>>();
            assert_eq!(puts, expected.into_iter().collect::<Vec<_>>());
        }
    }
}

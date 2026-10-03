//! Original ordered-join regressions; no upstream-method coverage credit.
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
type Row = (i32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
fn accepts(c: JoinComparison, a: i32, b: i32) -> bool {
    match c {
        JoinComparison::Equal => a == b,
        JoinComparison::LessThan => a < b,
        JoinComparison::LessThanOrEqual => a <= b,
        JoinComparison::GreaterThan => a > b,
        JoinComparison::GreaterThanOrEqual => a >= b,
    }
}
fn pairs(c: JoinComparison, left: &BTreeMap<usize, Row>, right: &BTreeMap<usize, Row>) -> Pairs {
    left.iter()
        .flat_map(|(&i, &a)| {
            right
                .iter()
                .filter(move |(_, b)| accepts(c, a.0, b.0))
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
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn extremes_duplicates_and_same_key_updates_retain_pairs_and_cached_keys() {
    for c in RELATIONS {
        let calls = Cell::new(0);
        let project = |r: &Row| {
            calls.set(calls.get() + 1);
            r.0
        };
        let mut join = JoinNode::comparing(c, project, project);
        let mut left = BTreeMap::new();
        let mut right = BTreeMap::new();
        let mut output = Pairs::new();
        for (id, key) in [i32::MIN, -1, 0, 0, 1, i32::MAX].into_iter().enumerate() {
            let row = (key, id as i64);
            left.insert(id, row);
            right.insert(id, row);
            apply(&mut output, join.apply_left(TupleChange::put(id, row)));
            apply(&mut output, join.apply_right(TupleChange::put(id, row)));
        }
        assert_eq!(output, pairs(c, &left, &right));
        for side in [false, true] {
            let row = (0, 500);
            let changes = if side {
                left.insert(2, row);
                join.apply_left(TupleChange::put(2, row))
            } else {
                right.insert(2, row);
                join.apply_right(TupleChange::put(2, row))
            };
            assert!(
                changes.iter().all(|c| c.value.is_some()),
                "same key only replaces"
            );
            apply(&mut output, changes);
            assert_eq!(output, pairs(c, &left, &right));
        }
        calls.set(0);
        for id in [0, 3, 5, 999] {
            left.remove(&id);
            right.remove(&id);
            apply(&mut output, join.apply_right(TupleChange::retract(id)));
            apply(&mut output, join.apply_left(TupleChange::retract(id)));
            assert_eq!(output, pairs(c, &left, &right));
        }
        assert_eq!(calls.get(), 0, "removal uses cached keys");
        assert!(!join.is_poisoned());
    }
}
#[test]
fn generated_bidirectional_edits_match_independent_cross_product() {
    let mut seed = 20261112;
    for c in RELATIONS {
        let mut join = JoinNode::comparing(c, |r: &Row| r.0, |r: &Row| r.0);
        let mut left = BTreeMap::new();
        let mut right = BTreeMap::new();
        let mut output = Pairs::new();
        for step in 0..1200 {
            let id = (random(&mut seed) % 24) as usize;
            let value = (!random(&mut seed).is_multiple_of(4))
                .then(|| ((random(&mut seed) % 9) as i32 - 4, step as i64));
            let change = TupleChange { id, value };
            let changes = if random(&mut seed).is_multiple_of(2) {
                apply(&mut left, [change.clone()]);
                join.apply_left(change)
            } else {
                apply(&mut right, [change.clone()]);
                join.apply_right(change)
            };
            apply(&mut output, changes);
            let expected = pairs(c, &left, &right);
            assert_eq!(output, expected, "{c:?} step {step}");
            assert_eq!(
                join.matches()
                    .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                    .collect::<Pairs>(),
                expected
            );
        }
    }
}
#[test]
fn shared_source_transactions_match_full_pairs_through_commit_undo_and_rebuild() {
    let mut seed = 20261113;
    for c in RELATIONS {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Row>();
        let joined = b
            .join_compare(source.port(), source.port(), c, |r| r.0, |r| r.0)
            .unwrap();
        let terminal = b
            .score("pairs", &joined, |(a, b)| SimpleScore::new(a.1 - b.1))
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
                        (random(&mut seed) % 7) as i32 - 3,
                        (random(&mut seed) % 31) as i64 - 15,
                    )
                });
                let change = TupleChange { id, value };
                apply(&mut next, [change.clone()]);
                t.apply(&source, change).unwrap();
                let expected = pairs(c, &next, &next)
                    .into_iter()
                    .map(|(id, (a, b))| (id, ((a, b), SimpleScore::new(a.1 - b.1))))
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(
                    t.graph().unwrap().constraint_matches(&terminal).unwrap(),
                    &expected
                );
                assert_eq!(
                    t.score().unwrap(),
                    SimpleScore::new(expected.values().map(|((a, b), _)| a.1 - b.1).sum())
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
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Counted {}
impl PartialOrd for Counted {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Counted {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.set(self.1.get() + 1);
        self.0.cmp(&other.0)
    }
}
#[test]
fn sparse_ranges_skip_nonmatching_buckets_on_either_side() {
    for (c, query, wanted) in [
        (JoinComparison::Equal, 2048, 2048),
        (JoinComparison::LessThan, 4094, 4095),
        (JoinComparison::LessThanOrEqual, 4095, 4095),
        (JoinComparison::GreaterThan, 1, 0),
        (JoinComparison::GreaterThanOrEqual, 0, 0),
    ] {
        for reversed in [false, true] {
            let count = Rc::new(Cell::new(0));
            let project = |r: &i32| Counted(*r, count.clone());
            let mut join =
                JoinNode::comparing(if reversed { c.reversed() } else { c }, project, project);
            for i in 0..4096 {
                let change = TupleChange::put(i, i);
                if reversed {
                    join.apply_left(change);
                } else {
                    join.apply_right(change);
                }
            }
            count.set(0);
            let change = TupleChange::put(-1, query);
            let output = if reversed {
                join.apply_right(change)
            } else {
                join.apply_left(change)
            };
            assert!(
                count.get() < 512,
                "{c:?} visited {} key comparisons",
                count.get()
            );
            let expected = if reversed {
                TupleChange::put((wanted, -1), (wanted, query))
            } else {
                TupleChange::put((-1, wanted), (query, wanted))
            };
            assert_eq!(output, vec![expected]);
        }
    }
}
#[test]
fn callback_errors_panics_and_failed_rebuild_restore_range_indexes() {
    let fault = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, i32>();
    let right = b.source::<usize, i32>();
    let joined = b
        .join_compare(
            left.port(),
            right.port(),
            JoinComparison::LessThan,
            |v| {
                assert_ne!(fault.get(), 3, "left key");
                *v
            },
            |v| {
                assert_ne!(fault.get(), 4, "right key");
                *v
            },
        )
        .unwrap();
    let terminal = b
        .score("pairs", &joined, |(_, r)| {
            calls.set(calls.get() + 1);
            assert!(
                !(fault.get() == 1 && *r == 1),
                "terminal panic after other pairs"
            );
            SimpleScore::new(1).with_structural(if fault.get() == 2 && *r == 1 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&left, TupleChange::put(0, 0)).unwrap();
    for (id, v) in [-1, 0, 1].into_iter().enumerate() {
        t.apply(&right, TupleChange::put(id, v)).unwrap();
    }
    t.commit().unwrap();
    let before = g.constraint_matches(&terminal).unwrap().clone();
    for mode in 1..=4 {
        fault.set(mode);
        calls.set(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let result = if mode == 4 {
                t.apply(&right, TupleChange::put(0, 2))
            } else {
                t.apply(&left, TupleChange::put(0, -2))
            };
            assert!(result.is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(outcome.is_err(), mode != 2);
        if mode <= 2 {
            assert_eq!(calls.get(), 3, "earlier matches propagated before failure");
        }
        fault.set(0);
        assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
        assert_eq!(g.source_rows(&left).unwrap()[&0], 0);
        assert_eq!(g.source_rows(&right).unwrap()[&0], -1);
        g.assert_consistent().unwrap();
    }
    fault.set(3);
    assert!(catch_unwind(AssertUnwindSafe(|| g.assert_consistent())).is_err());
    fault.set(0);
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    let mut t = g.transaction();
    t.apply(&left, TupleChange::put(0, -2)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
}
#[test]
fn foreign_ports_and_relation_reversal_keep_orientation_explicit() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i32>();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = other.source::<usize, i32>();
    for c in RELATIONS {
        assert!(matches!(
            b.join_compare(source.port(), foreign.port(), c, |v| *v, |v| *v),
            Err(GraphError::ForeignPort)
        ));
        assert!(matches!(
            b.join_compare(foreign.port(), source.port(), c, |v| *v, |v| *v),
            Err(GraphError::ForeignPort)
        ));
        assert_eq!(c.reversed().reversed(), c);
        for a in -2..=2 {
            for d in -2..=2 {
                assert_eq!(c.matches(&a, &d), c.reversed().matches(&d, &a));
            }
        }
    }
    let port = b
        .join_compare(
            source.port(),
            source.port(),
            JoinComparison::LessThan,
            |v| *v,
            |v| *v,
        )
        .unwrap();
    b.score("pairs", &port, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, 1)).unwrap();
    t.apply(&source, TupleChange::put(1, 2)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-1));
    g.assert_consistent().unwrap();
}
#[test]
fn construction_and_search_find_strictly_increasing_values() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let port = b
            .join_compare(
                &assigned,
                &assigned,
                JoinComparison::GreaterThanOrEqual,
                |r| r.1.unwrap(),
                |r| r.1.unwrap(),
            )
            .unwrap();
        let ordered = b.filter(&port, |(a, b)| a.0 < b.0).unwrap();
        b.score("inversions", &ordered, |_| SimpleScore::new(-1))
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
    assert_eq!(c.score, Some(SimpleScore::ZERO));
    assert_eq!(c.assignment.values(), &[Some(0), Some(1), Some(2)]);
    let p = FiniteProblem::new(
        (0..3)
            .map(|i| Variable::new(vec![0, 1, 2]).with_initial(2 - i))
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
            max_steps: 20,
            max_evaluations: 200,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert_eq!(best.assignment.values(), &[Some(0), Some(1), Some(2)]);
    let v = best.assignment.values();
    let inversions = (0..3)
        .flat_map(|i| (i + 1..3).map(move |j| (i, j)))
        .filter(|&(i, j)| v[i] >= v[j])
        .count();
    assert_eq!(best.score, SimpleScore::new(-(inversions as i64)));
    assert_eq!(best.score, SimpleScore::ZERO);
}
#[test]
fn group_sum_changes_move_between_ranges_and_undo_restores_old_groups() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let limits = b.source::<usize, i64>();
    let groups = b.group_collect(source.port(), |r| *r).unwrap();
    let port = b
        .join_compare(
            &groups,
            limits.port(),
            JoinComparison::GreaterThan,
            |(_, s)| s.sum,
            |v| *v,
        )
        .unwrap();
    let terminal = b
        .score("over", &port, |((_, s), limit)| {
            SimpleScore::new(limit - s.sum)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, v) in [(0, 5), (1, 10)] {
        t.apply(&limits, TupleChange::put(id, v)).unwrap();
    }
    for (id, row) in [(0, (0, 4)), (1, (0, 3)), (2, (1, 12))] {
        t.apply(&source, TupleChange::put(id, row)).unwrap();
    }
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-11));
    let before = g.constraint_matches(&terminal).unwrap().clone();
    assert_eq!(
        before.keys().copied().collect::<Vec<_>>(),
        vec![(0, 0), (1, 0), (1, 1)]
    );
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(1, (1, -4))).unwrap();
        let matches = t.graph().unwrap().constraint_matches(&terminal).unwrap();
        assert_eq!(matches.keys().copied().collect::<Vec<_>>(), vec![(1, 0)]);
        assert_eq!(matches[&(1, 0)].0.0.1.sum, 8);
        assert_eq!(t.score().unwrap(), SimpleScore::new(-3));
        t.apply(&source, TupleChange::retract(2)).unwrap();
        assert!(
            t.graph()
                .unwrap()
                .constraint_matches(&terminal)
                .unwrap()
                .is_empty()
        );
        t.apply(&source, TupleChange::retract(1)).unwrap();
        t.assert_consistent().unwrap();
    }
    assert_eq!(g.constraint_matches(&terminal).unwrap(), &before);
    g.assert_consistent().unwrap();
}

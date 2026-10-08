//! Original compact join-round regressions against independent pair scans.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (i32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
const COMPARISONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
fn scan(l: &BTreeMap<usize, Row>, r: &BTreeMap<usize, Row>, c: JoinComparison) -> Pairs {
    l.iter()
        .flat_map(|(&i, &a)| {
            r.iter().filter_map(move |(&j, &b)| {
                let matched = match c {
                    JoinComparison::Equal => a.0 == b.0,
                    JoinComparison::LessThan => a.0 < b.0,
                    JoinComparison::LessThanOrEqual => a.0 <= b.0,
                    JoinComparison::GreaterThan => a.0 > b.0,
                    JoinComparison::GreaterThanOrEqual => a.0 >= b.0,
                };
                matched.then_some(((i, j), (a, b)))
            })
        })
        .collect()
}
fn apply_rows(rows: &mut BTreeMap<usize, Row>, changes: &[TupleChange<usize, Row>]) {
    for c in changes {
        if let Some(v) = c.value {
            rows.insert(c.id, v);
        } else {
            rows.remove(&c.id);
        }
    }
}
fn changes(seed: &mut u64) -> Vec<TupleChange<usize, Row>> {
    (0..5)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = *seed >> 32;
            TupleChange {
                id: (x % 8) as usize,
                value: (!x.is_multiple_of(4)).then_some(((x / 8 % 5) as i32, (x / 40 % 17) as i64)),
            }
        })
        .collect()
}
#[test]
fn standalone_rounds_match_scans_and_interoperate_with_immediate_updates() {
    for c in COMPARISONS {
        let mut n = JoinNode::comparing(c, |r: &Row| r.0, |r: &Row| r.0);
        let (mut l, mut r, mut visible) = (BTreeMap::new(), BTreeMap::new(), Pairs::new());
        let mut seed = 197109u64;
        for round in 0..240 {
            if round % 11 == 0 {
                let single = TupleChange::put(0, (round % 5, round as i64));
                apply_rows(&mut l, std::slice::from_ref(&single));
                for e in n.apply_left(single) {
                    if let Some(v) = e.value {
                        visible.insert(e.id, v);
                    } else {
                        assert!(visible.remove(&e.id).is_some());
                    }
                }
            }
            let (left, right) = (changes(&mut seed), changes(&mut seed));
            apply_rows(&mut l, &left);
            apply_rows(&mut r, &right);
            let output = n.apply_batch(left, right);
            let mut seen = BTreeSet::new();
            for e in output {
                assert!(seen.insert(e.id), "more than one event per pair");
                if let Some(v) = e.value {
                    visible.insert(e.id, v);
                } else {
                    assert!(visible.remove(&e.id).is_some(), "spurious retract");
                }
            }
            let expected = scan(&l, &r, c);
            assert_eq!(visible, expected);
            assert_eq!(
                n.matches()
                    .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
                    .collect::<Pairs>(),
                expected
            );
        }
    }
}
#[test]
fn graph_rounds_preserve_activity_and_rollback_for_all_comparisons() {
    for comparison in COMPARISONS {
        for pruning in [false, true] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let left = b.source::<usize, Row>();
            let right = b.source::<usize, Row>();
            let joined = b
                .join_compare_batched(left.port(), right.port(), comparison, |v| v.0, |v| v.0)
                .unwrap();
            let data = b.dataset(&joined).unwrap();
            b.score("pairs", &joined, |(a, b)| SimpleScore::new(a.1 + b.1))
                .unwrap();
            let mut g = b.build();
            let (mut l, mut r) = (BTreeMap::new(), BTreeMap::new());
            let mut seed = 1705;
            for round in 0..100 {
                let (lc, rc) = if round % 13 == 0 {
                    (Vec::new(), (0..8).map(TupleChange::retract).collect())
                } else {
                    (changes(&mut seed), changes(&mut seed))
                };
                let (mut nl, mut nr) = (l.clone(), r.clone());
                apply_rows(&mut nl, &lc);
                apply_rows(&mut nr, &rc);
                let mut batch = StreamBatch::new();
                for c in lc {
                    batch.push(&left, c).unwrap();
                }
                for c in rc {
                    batch.push(&right, c).unwrap();
                }
                let mut tx = g.transaction();
                tx.apply_batch(batch).unwrap();
                let want = scan(&nl, &nr, comparison);
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(want.values().map(|(a, b)| a.1 + b.1).sum())
                );
                assert_eq!(
                    tx.graph()
                        .unwrap()
                        .dataset_rows(&data)
                        .unwrap()
                        .iter()
                        .map(|(i, v)| (*i, *v))
                        .collect::<Pairs>(),
                    want
                );
                if round % 4 == 0 {
                    drop(tx);
                } else {
                    tx.commit().unwrap();
                    l = nl;
                    r = nr;
                }
                assert_eq!(
                    g.dataset_rows(&data)
                        .unwrap()
                        .iter()
                        .map(|(i, v)| (*i, *v))
                        .collect::<Pairs>(),
                    scan(&l, &r, comparison)
                );
            }
            g.assert_consistent().unwrap();
        }
    }
}
#[test]
fn self_join_scores_only_final_payloads_and_restores_after_sink_panic() {
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<usize, Row>();
    let joined = b
        .join_equal_batched(source.port(), source.port(), |r| r.0, |r| r.0)
        .unwrap();
    let data = b.dataset(&joined).unwrap();
    b.score("pairs", &joined, |(l, r)| {
        calls.set(calls.get() + 1);
        assert_eq!(l.1 % 2, r.1 % 2, "intermediate self-join payload");
        assert!(!fail.get(), "sink failure");
        SimpleScore::new(l.1 + r.1)
    })
    .unwrap();
    let mut g = b.build();
    let batch = |a, b| {
        let mut changes = StreamBatch::new();
        changes.push(&source, TupleChange::put(0, (0, a))).unwrap();
        changes.push(&source, TupleChange::put(1, (0, b))).unwrap();
        changes
    };
    let mut tx = g.transaction();
    tx.apply_batch(batch(2, 4)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 4);
    let original = g.dataset_rows(&data).unwrap().clone();
    let score = g.score();
    let mut tx = g.transaction();
    tx.apply_batch(batch(3, 5)).unwrap();
    assert_eq!(calls.get(), 8);
    drop(tx);
    assert_eq!(g.score(), score);
    assert_eq!(g.dataset_rows(&data).unwrap(), &original);
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply_batch(batch(3, 5)).unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(g.score(), score);
    assert_eq!(g.dataset_rows(&data).unwrap(), &original);
    let mut tx = g.transaction();
    tx.apply_batch(batch(3, 5)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(32));
    g.assert_consistent().unwrap();
}
#[test]
fn projections_use_last_rows_and_input_or_projection_panics_leave_state_unchanged() {
    let calls = Cell::new(0);
    let project = |r: &Row| {
        calls.set(calls.get() + 1);
        assert!(r.1 >= 0);
        r.0
    };
    let mut n = JoinNode::new(project, project);
    let result = n.apply_batch(
        [TupleChange::put(0, (0, -1)), TupleChange::put(0, (1, 2))],
        [TupleChange::put(0, (1, 4))],
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(result, vec![TupleChange::put((0, 0), ((1, 2), (1, 4)))]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| n.apply_batch(
            [TupleChange::put(0, (2, 2))],
            [TupleChange::put(0, (2, -1))]
        )))
        .is_err()
    );
    let mut pulls = 0;
    let input = std::iter::from_fn(|| {
        pulls += 1;
        assert!(pulls == 1, "iterator failure");
        Some(TupleChange::put(0, (3, 2)))
    });
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply_batch(input, []))).is_err());
    assert!(!n.is_poisoned());
    assert_eq!(
        n.matches()
            .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
            .collect::<Vec<_>>(),
        vec![((0, 0), ((1, 2), (1, 4)))]
    );
    assert_eq!(
        n.apply_batch([TupleChange::retract(0), TupleChange::put(0, (1, 6))], []),
        vec![TupleChange::put((0, 0), ((1, 6), (1, 4)))]
    );
}

fn tick(fault: &Cell<Option<usize>>) {
    if let Some(n) = fault.get() {
        assert!(n > 0, "injected failure");
        fault.set(Some(n - 1));
    }
}
#[derive(Clone)]
struct Key(i32, Rc<Cell<Option<usize>>>);
impl PartialEq for Key {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
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
        tick(&self.1);
        self.0.cmp(&r.0)
    }
}
struct Payload(i32, Rc<Cell<Option<usize>>>);
impl Clone for Payload {
    fn clone(&self) -> Self {
        tick(&self.1);
        Self(self.0, self.1.clone())
    }
}
#[test]
fn comparison_and_payload_clone_failures_preserve_committed_indexes() {
    for comparison_fault in [false, true] {
        let (mut failures, mut successes) = (0, 0);
        for budget in 0..100 {
            let fault = Rc::new(Cell::new(None));
            let quiet = Rc::new(Cell::new(None));
            let key_fault = if comparison_fault {
                fault.clone()
            } else {
                quiet.clone()
            };
            let clone_fault = if comparison_fault {
                quiet
            } else {
                fault.clone()
            };
            let project = |p: &Payload| Key(p.0, key_fault.clone());
            let mut n = JoinNode::comparing(JoinComparison::LessThanOrEqual, project, project);
            n.apply_batch(
                (0..3).map(|i| TupleChange::put(i, Payload(i, clone_fault.clone()))),
                (0..3).map(|i| TupleChange::put(i, Payload(i, clone_fault.clone()))),
            );
            let before = n
                .matches()
                .map(|((i, j), (l, r))| ((*i, *j), (l.0, r.0)))
                .collect::<BTreeMap<_, _>>();
            fault.set(Some(budget));
            let result = catch_unwind(AssertUnwindSafe(|| {
                n.apply_batch(
                    [
                        TupleChange::put(0, Payload(2, clone_fault.clone())),
                        TupleChange::retract(1),
                    ],
                    [TupleChange::put(0, Payload(3, clone_fault.clone()))],
                )
            }));
            fault.set(None);
            assert!(!n.is_poisoned());
            let after = n
                .matches()
                .map(|((i, j), (l, r))| ((*i, *j), (l.0, r.0)))
                .collect::<BTreeMap<_, _>>();
            if result.is_err() {
                failures += 1;
                assert_eq!(after, before);
            } else {
                successes += 1;
                assert_eq!(
                    after,
                    BTreeMap::from([
                        ((0, 0), (2, 3)),
                        ((0, 2), (2, 2)),
                        ((2, 0), (2, 3)),
                        ((2, 2), (2, 2))
                    ])
                );
            }
        }
        assert!(failures > 0 && successes > 0);
    }
}

#[test]
fn dense_join_retains_input_payloads_without_an_expanded_pair_cache() {
    let live = Rc::new(());
    let key = |r: &(i32, Rc<()>)| r.0;
    let mut join = JoinNode::new(key, key);
    let output = join.apply_batch(
        (0..25).map(|i| TupleChange::put(i, (0, live.clone()))),
        (0..35).map(|i| TupleChange::put(i, (0, live.clone()))),
    );
    assert_eq!(output.len(), 875);
    drop(output);
    // One external owner plus exactly one payload per retained left/right row.
    assert_eq!(Rc::strong_count(&live), 61);
    assert_eq!(join.matches().count(), 875);
    drop(join);
    assert_eq!(Rc::strong_count(&live), 1);
}

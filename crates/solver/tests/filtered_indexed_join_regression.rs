//! Original compact filtered-indexed join regressions; no upstream credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
type Row = (i32, i64);
fn relation(c: JoinComparison, l: i32, r: i32) -> bool {
    match c {
        JoinComparison::Equal => l == r,
        JoinComparison::LessThan => l < r,
        JoinComparison::LessThanOrEqual => l <= r,
        JoinComparison::GreaterThan => l > r,
        JoinComparison::GreaterThanOrEqual => l >= r,
    }
}
#[test]
fn all_relations_match_scans_and_evaluate_each_affected_final_pair_once() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        for pruning in [false, true] {
            let calls = Cell::new(0);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let left = b.source::<usize, Row>();
            let right = b.source::<usize, Row>();
            let joined = b
                .join_compare_filtered_batched(
                    left.port(),
                    right.port(),
                    comparison,
                    |r| r.0,
                    |r| r.0,
                    |l, r| {
                        calls.set(calls.get() + 1);
                        (l.1 - r.1).rem_euclid(3) == 0
                    },
                )
                .unwrap();
            let dataset = b.dataset(&joined).unwrap();
            b.score("pairs", &joined, |(l, r)| SimpleScore::new(l.1 + r.1))
                .unwrap();
            let mut g = b.build();
            let mut l = BTreeMap::new();
            let mut r = BTreeMap::new();
            let mut random = 617u64;
            for round in 0..120 {
                let mut next_l = l.clone();
                let mut next_r = r.clone();
                let mut changed_l = BTreeSet::new();
                let mut changed_r = BTreeSet::new();
                let mut batch = StreamBatch::new();
                for _ in 0..7 {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let id = (random >> 16) as usize % 9;
                    let value = (!random.is_multiple_of(5)).then_some((
                        ((random >> 27) % 5) as i32,
                        ((random >> 40) % 17) as i64 - 8,
                    ));
                    let (rows, changed, source) = if random & 1 == 0 {
                        (&mut next_l, &mut changed_l, &left)
                    } else {
                        (&mut next_r, &mut changed_r, &right)
                    };
                    changed.insert(id);
                    if let Some(value) = value {
                        rows.insert(id, value);
                    } else {
                        rows.remove(&id);
                    }
                    batch.push(source, TupleChange { id, value }).unwrap();
                }
                let mut expected = BTreeMap::new();
                let mut expected_calls = 0;
                for (&i, a) in &next_l {
                    for (&j, b) in &next_r {
                        if relation(comparison, a.0, b.0) {
                            expected_calls +=
                                usize::from(changed_l.contains(&i) || changed_r.contains(&j));
                            if (a.1 - b.1).rem_euclid(3) == 0 {
                                expected.insert((i, j), (*a, *b));
                            }
                        }
                    }
                }
                let before = g.dataset_rows(&dataset).unwrap().clone();
                calls.set(0);
                let mut trial = g.transaction();
                trial.apply_batch(batch).unwrap();
                if !pruning {
                    assert_eq!(calls.get(), expected_calls);
                }
                assert_eq!(
                    trial.graph().unwrap().dataset_rows(&dataset).unwrap(),
                    &expected
                );
                assert_eq!(
                    trial.score().unwrap(),
                    SimpleScore::new(expected.values().map(|(l, r)| l.1 + r.1).sum())
                );
                if round % 11 == 0 {
                    trial.assert_consistent().unwrap();
                }
                if round % 3 == 0 {
                    drop(trial);
                    assert_eq!(g.dataset_rows(&dataset).unwrap(), &before);
                } else {
                    trial.commit().unwrap();
                    l = next_l;
                    r = next_r;
                }
            }
        }
    }
}
struct CountedRow {
    value: i32,
    clones: Rc<Cell<usize>>,
}
impl Clone for CountedRow {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
#[test]
fn rejecting_pairs_never_clones_unchanged_opposite_payloads() {
    let left_clones = Rc::new(Cell::new(0));
    let right_clones = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, CountedRow>();
    let right = b.source::<usize, CountedRow>();
    let joined = b
        .join_batched(
            left.port(),
            right.port(),
            |_| (),
            |_| (),
            |_, _| {
                calls.set(calls.get() + 1);
                false
            },
        )
        .unwrap();
    b.score("never", &joined, |_| panic!("rejected pair reached score"))
        .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch
        .push(
            &left,
            TupleChange::put(
                0,
                CountedRow {
                    value: 0,
                    clones: left_clones.clone(),
                },
            ),
        )
        .unwrap();
    for id in 0..32 {
        batch
            .push(
                &right,
                TupleChange::put(
                    id,
                    CountedRow {
                        value: id as i32,
                        clones: right_clones.clone(),
                    },
                ),
            )
            .unwrap();
    }
    let mut trial = g.transaction();
    trial.apply_batch(batch).unwrap();
    trial.commit().unwrap();
    right_clones.set(0);
    calls.set(0);
    {
        let mut trial = g.transaction();
        trial
            .apply(
                &left,
                TupleChange::put(
                    0,
                    CountedRow {
                        value: 9,
                        clones: left_clones,
                    },
                ),
            )
            .unwrap();
        assert_eq!(calls.get(), 32);
        assert_eq!(right_clones.get(), 0);
        assert_eq!(trial.score().unwrap(), SimpleScore::ZERO);
    }
    assert_eq!(right_clones.get(), 0);
    assert_eq!(g.score(), SimpleScore::ZERO);
}
#[derive(Clone)]
struct Id(usize, Rc<Cell<usize>>);
impl PartialEq for Id {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Id {
    fn cmp(&self, r: &Self) -> Ordering {
        let left = self.1.get();
        if left > 0 {
            self.1.set(left - 1);
            assert_ne!(left, 1, "comparison failure");
        }
        self.0.cmp(&r.0)
    }
}
#[test]
fn every_comparison_failure_restores_inputs_and_accepted_ids_without_comparisons() {
    let budget = Rc::new(Cell::new(0));
    let id = |i| Id(i, budget.clone());
    let fresh = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let left = b.source::<Id, i32>();
        let right = b.source::<Id, i32>();
        let joined = b
            .join_batched(
                left.port(),
                right.port(),
                |_| (),
                |_| (),
                |l, r| (l + r) % 2 == 0,
            )
            .unwrap();
        let dataset = b.dataset(&joined).unwrap();
        let mut g = b.build();
        let mut batch = StreamBatch::new();
        for i in 0..3 {
            batch
                .push(&left, TupleChange::put(id(i), i as i32))
                .unwrap();
            batch
                .push(&right, TupleChange::put(id(i), i as i32))
                .unwrap();
        }
        let mut t = g.transaction();
        t.apply_batch(batch).unwrap();
        t.commit().unwrap();
        (g, left, right, dataset)
    };
    let batch = |left: &StreamSource<Id, i32>, right: &StreamSource<Id, i32>| {
        let mut b = StreamBatch::new();
        b.push(left, TupleChange::put(id(0), 10)).unwrap();
        b.push(right, TupleChange::put(id(1), 4)).unwrap();
        b
    };
    let (mut g, left, right, dataset) = fresh();
    let rows = |g: &StreamGraph<'_, SimpleScore>, data: &StreamDataset<(Id, Id), (i32, i32)>| {
        g.dataset_rows(data)
            .unwrap()
            .iter()
            .map(|((i, j), v)| ((i.0, j.0), *v))
            .collect::<Vec<_>>()
    };
    let before = rows(&g, &dataset);
    let mut trial = g.transaction();
    budget.set(usize::MAX);
    trial.apply_batch(batch(&left, &right)).unwrap();
    let count = usize::MAX - budget.get();
    budget.set(0);
    let expected = rows(trial.graph().unwrap(), &dataset);
    drop(trial);
    assert!(count > 0);
    for failure in 1..=count {
        let (mut g, left, right, dataset) = fresh();
        let mut trial = g.transaction();
        budget.set(failure);
        assert!(
            catch_unwind(AssertUnwindSafe(|| trial.apply_batch(batch(&left, &right)))).is_err()
        );
        budget.set(1);
        drop(trial);
        assert_eq!(budget.get(), 1);
        budget.set(0);
        assert_eq!(rows(&g, &dataset), before);
        assert_eq!(g.source_rows(&left).unwrap()[&id(0)], 0);
        assert_eq!(g.source_rows(&right).unwrap()[&id(1)], 1);
        let mut trial = g.transaction();
        trial.apply_batch(batch(&left, &right)).unwrap();
        trial.commit().unwrap();
        assert_eq!(rows(&g, &dataset), expected);
        g.assert_consistent().unwrap();
    }
}

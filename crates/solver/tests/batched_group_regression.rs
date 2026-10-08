//! Original batched-group behavioral and atomicity regressions.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn scan(rows: &BTreeMap<usize, (usize, i64)>) -> BTreeMap<usize, IntSummary> {
    let mut values = BTreeMap::<usize, Vec<i64>>::new();
    for &(key, value) in rows.values() {
        values.entry(key).or_default().push(value);
    }
    values
        .into_iter()
        .map(|(key, mut values)| {
            values.sort();
            let count = values.len();
            let sum = values.iter().fold(0i64, |sum, v| sum.wrapping_add(*v));
            let min = values.first().copied();
            let max = values.last().copied();
            values.dedup();
            (
                key,
                IntSummary {
                    count,
                    distinct_count: values.len(),
                    sum,
                    min,
                    max,
                },
            )
        })
        .collect()
}
#[test]
fn rounds_match_full_scans_and_publish_each_final_group_once() {
    let mut node = GroupNode::new();
    let mut rows = BTreeMap::new();
    let mut published = BTreeMap::new();
    let mut random = 91u64;
    let extremes = [i64::MIN, i64::MAX, 0, 1, -1, 5];
    for _ in 0..300 {
        let mut changes = Vec::new();
        for _ in 0..9 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = (random >> 17) as usize % 24;
            let key = (random >> 35) as usize % 5;
            if random.is_multiple_of(4) {
                rows.remove(&id);
                changes.push(TupleChange::retract(id));
            } else {
                let value = extremes[(random >> 41) as usize % extremes.len()];
                rows.insert(id, (key, value));
                changes.push(TupleChange::put(id, (key, value)));
            }
        }
        let changes = node.apply_batch(changes);
        assert!(changes.windows(2).all(|pair| pair[0].id < pair[1].id));
        for change in changes {
            if let Some(value) = change.value {
                published.insert(change.id, value);
            } else {
                assert!(published.remove(&change.id).is_some());
            }
        }
        assert_eq!(published, scan(&rows));
        assert_eq!(
            node.groups()
                .map(|(k, v)| (*k, v))
                .collect::<BTreeMap<_, _>>(),
            published
        );
        assert!(!node.is_poisoned());
    }
    let absent = 1000;
    assert!(
        node.apply_batch([
            TupleChange::put(absent, (999, 100)),
            TupleChange::retract(absent)
        ])
        .is_empty()
    );
}

#[derive(Clone, Debug)]
struct Key(usize, usize, Rc<Cell<usize>>);
impl PartialEq for Key {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, r: &Self) -> Option<Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Key {
    fn cmp(&self, r: &Self) -> Ordering {
        let left = self.2.get();
        if left > 0 {
            self.2.set(left - 1);
            assert_ne!(left, 1, "comparison failure");
        }
        self.0.cmp(&r.0)
    }
}
#[test]
fn stored_group_representatives_survive_rounds_and_new_keys_follow_input_order() {
    let budget = Rc::new(Cell::new(0));
    let key = |value, tag| Key(value, tag, budget.clone());
    let mut node = GroupNode::new();
    node.apply_batch([TupleChange::put(0, (key(1, 10), 5))]);
    let output = node.apply_batch([
        TupleChange::retract(0),
        TupleChange::put(1, (key(1, 20), 9)),
    ]);
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].id.1, 10);
    assert_eq!(node.groups().next().unwrap().0.1, 10);
    let output = node.apply_batch([
        TupleChange::put(4, (key(2, 40), 1)),
        TupleChange::put(3, (key(2, 30), 2)),
    ]);
    assert_eq!(output[0].id.1, 30);
    assert_eq!(output[0].value.as_ref().unwrap().sum, 3);
}
#[test]
fn every_comparison_panic_preserves_groups_inputs_and_recovery() {
    let budget = Rc::new(Cell::new(0));
    let key = |value| Key(value, value, budget.clone());
    let fresh = || {
        let mut node = GroupNode::new();
        node.apply_batch((0..6).map(|i| TupleChange::put(key(i), (key(i % 2), i as i64))));
        node
    };
    let changes = || {
        vec![
            TupleChange::put(key(0), (key(1), 100)),
            TupleChange::retract(key(2)),
            TupleChange::put(key(7), (key(4), 77)),
        ]
    };
    let observed = |node: &GroupNode<Key, Key>| {
        node.groups()
            .map(|(k, v)| (k.0, v))
            .collect::<BTreeMap<_, _>>()
    };
    let mut node = fresh();
    let original = observed(&node);
    budget.set(usize::MAX);
    node.apply_batch(changes());
    let count = usize::MAX - budget.get();
    budget.set(0);
    let expected = observed(&node);
    assert!(count > 0);
    for failure in 1..=count {
        let mut node = fresh();
        budget.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| node.apply_batch(changes()))).is_err());
        budget.set(0);
        assert!(!node.is_poisoned());
        assert_eq!(observed(&node), original);
        node.apply_batch(changes());
        assert_eq!(observed(&node), expected);
        node.apply_batch((0..8).map(|i| TupleChange::retract(key(i))));
        assert!(node.groups().next().is_none());
    }
}
#[test]
fn solver_batches_publish_final_scores_and_undo_mapping_and_sink_failures() {
    for pruning in [false, true] {
        let calls = Cell::new(0);
        let failure = Cell::new(0);
        let problem = FiniteProblem::new(vec![
            Variable::new(vec![1i64, 2]).with_initial(1),
            Variable::new(vec![1, 2]).with_initial(1),
        ])
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, (usize, Option<i64>)>();
        let groups = b
            .group_collect_batched(source.port(), |r| {
                assert_ne!(failure.get(), 1, "mapping failure");
                ((), r.1.unwrap())
            })
            .unwrap();
        let dataset = b.dataset(&groups).unwrap();
        b.score("sum", &groups, |(_, stats)| {
            calls.set(calls.get() + 1);
            assert_ne!(failure.get(), 2, "score failure");
            assert_eq!(stats.sum % 2, 0);
            SimpleScore::new(stats.sum)
        })
        .unwrap();
        let mut director =
            StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
                .unwrap();
        assert_eq!(director.score(), SimpleScore::new(2));
        assert_eq!(calls.get(), 1);
        let movement = Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 1,
                value: Some(2),
            },
        ]);
        calls.set(0);
        {
            let trial = director.apply(&movement).unwrap();
            assert_eq!(trial.score(), SimpleScore::new(4));
            assert_eq!(trial.constraint_evaluations(), 1);
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(director.score(), SimpleScore::new(2));
        for fail in [1, 2] {
            failure.set(fail);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _trial = director.apply(&movement);
                }))
                .is_err()
            );
            failure.set(0);
            assert_eq!(director.score(), SimpleScore::new(2));
            director.assert_consistent().unwrap();
        }
        director.apply(&movement).unwrap().commit();
        assert_eq!(director.score(), SimpleScore::new(4));
        director.assert_consistent().unwrap();
        assert_eq!(
            director.graph().dataset_rows(&dataset).unwrap()[&()].1.sum,
            4
        );
    }
}

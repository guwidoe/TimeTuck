//! Original source-ID, arithmetic, ownership and graph integration regressions.
use super::*;
use crate::*;
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn observed(node: &SumGroupNode<usize, usize>) -> BTreeMap<usize, i64> {
    node.groups().map(|(key, sum)| (*key, sum)).collect()
}

fn scan(rows: &BTreeMap<usize, (usize, i64)>, wrapping: bool) -> BTreeMap<usize, i64> {
    let mut groups = BTreeMap::<usize, i128>::new();
    for &(key, value) in rows.values() {
        *groups.entry(key).or_default() += i128::from(value);
    }
    groups
        .into_iter()
        .map(|(key, sum)| {
            let total = if wrapping {
                sum as i64
            } else {
                i64::try_from(sum).unwrap()
            };
            (key, total)
        })
        .collect()
}

#[test]
fn checked_boundaries_preserve_captures_and_both_replacement_intermediates() {
    let edge = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    for old in edge {
        for new in edge {
            let mut node = SumGroupNode::new();
            node.apply(TupleChange::put(0usize, (0usize, old)));
            let original = node.clone();
            let insertion =
                i64::try_from(i128::from(old) + i128::from(new)).map_err(|_| SumOverflow);
            let actual = node.try_apply(TupleChange::put(1, (0, new)));
            assert_eq!(
                actual
                    .as_ref()
                    .map(|rows| rows[0].value.unwrap())
                    .map_err(|e| *e),
                insertion
            );
            if insertion.is_err() {
                assert_eq!(observed(&node), observed(&original));
                assert_eq!(node.input(&0), original.input(&0));
                assert!(node.input(&1).is_none());
            }
            let mut node = original.clone();
            let replacement = i64::try_from(i128::from(new) - i128::from(old))
                .map(|_| new)
                .map_err(|_| SumOverflow);
            let actual = node.try_apply(TupleChange::put(0, (0, new)));
            assert_eq!(actual.map(|rows| rows[0].value.unwrap()), replacement);
            if replacement.is_err() {
                assert_eq!(observed(&node), observed(&original));
                assert_eq!(node.input(&0), original.input(&0));
            }
            assert!(node.try_apply(TupleChange::retract(99)).unwrap().is_empty());
        }
    }
    let mut node = SumGroupNode::new();
    for (id, value) in [-1, i64::MAX, 1, 0, 0].into_iter().enumerate() {
        node.apply(TupleChange::put(id, (0, value)));
    }
    let original = node.clone();
    // Final replacement difference fits, but adding it to the total does not.
    assert_eq!(
        node.try_apply(TupleChange::put(2, (0, 2))),
        Err(SumOverflow)
    );
    // Removing a negative captured value would overflow the total.
    assert_eq!(node.try_apply(TupleChange::retract(0)), Err(SumOverflow));
    assert_eq!(observed(&node), observed(&original));
    for id in 0..5 {
        assert_eq!(node.input(&id), original.input(&id));
    }
    node.apply(TupleChange::put(0, (0, -2)));
    for id in [3, 4, 2, 1, 0] {
        node.apply(TupleChange::retract(id));
    }
    assert!(node.groups().next().is_none());
}

#[test]
fn final_id_order_checks_intermediates_and_ignores_superseded_payloads() {
    let mut node = SumGroupNode::new();
    node.apply(TupleChange::put(0usize, (0usize, i64::MIN)));
    // The final equal replacement fits both checked intermediates; MAX is never applied.
    assert_eq!(
        node.try_apply_batch([
            TupleChange::put(0, (0, i64::MAX)),
            TupleChange::put(0, (0, i64::MIN)),
        ]),
        Ok(vec![TupleChange::put(0, i64::MIN)])
    );
    let mut node = SumGroupNode::new();
    node.apply(TupleChange::put(0usize, (0usize, i64::MAX)));
    // A superseded retraction does not remove the old captured value.
    assert_eq!(
        node.try_apply_batch([
            TupleChange::retract(0),
            TupleChange::put(1, (0, i64::MAX)),
            TupleChange::put(0, (0, 1)),
        ]),
        Err(SumOverflow)
    );
    assert_eq!(node.input(&0), Some(&(0, i64::MAX)));
    assert!(node.input(&1).is_none());
    assert!(
        node.apply_batch([
            TupleChange::put(99, (0, i64::MAX)),
            TupleChange::retract(99),
        ])
        .is_empty()
    );
    let mut node = SumGroupNode::new();
    node.apply(TupleChange::put(2usize, (0usize, i64::MAX)));
    // The final total is zero, but sorted ID 0 overflows before removal of ID 2.
    assert_eq!(
        node.try_apply_batch([
            TupleChange::retract(2),
            TupleChange::put(0, (0, 1)),
            TupleChange::put(1, (0, -1)),
        ]),
        Err(SumOverflow)
    );
    assert_eq!(node.input(&2), Some(&(0, i64::MAX)));
}

#[test]
fn rounds_and_immediate_updates_match_independent_source_id_sums() {
    for wrapping in [false, true] {
        let mut node = if wrapping {
            SumGroupNode::wrapping()
        } else {
            SumGroupNode::new()
        };
        let mut rows = BTreeMap::new();
        let mut published = BTreeMap::new();
        let mut random = 267u64;
        let extremes = [i64::MIN, i64::MAX, -1, 0, 1, 7];
        for round in 0..320 {
            let before = node.clone();
            let prior_rows = rows.clone();
            let mut changes = Vec::new();
            for _ in 0..if round % 3 == 0 { 1 } else { 12 } {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let id = (random >> 17) as usize % 64;
                let key = (random >> 35) as usize % 7;
                if random.is_multiple_of(4) {
                    rows.remove(&id);
                    changes.push(TupleChange::retract(id));
                } else {
                    let value = if wrapping {
                        extremes[(random >> 41) as usize % extremes.len()]
                    } else {
                        ((random >> 41) % 2001) as i64 - 1000
                    };
                    rows.insert(id, (key, value));
                    changes.push(TupleChange::put(id, (key, value)));
                }
            }
            let output = if changes.len() == 1 {
                node.try_apply(changes.pop().unwrap()).unwrap()
            } else {
                node.try_apply_batch(changes).unwrap()
            };
            assert!(output.windows(2).all(|pair| pair[0].id < pair[1].id));
            for change in output {
                if let Some(sum) = change.value {
                    published.insert(change.id, sum);
                } else {
                    assert!(published.remove(&change.id).is_some());
                }
            }
            assert_eq!(published, scan(&rows, wrapping));
            assert_eq!(observed(&node), published);
            assert_eq!(observed(&before), scan(&prior_rows, wrapping));
            for id in 0..64 {
                assert_eq!(node.input(&id), rows.get(&id));
                assert_eq!(before.input(&id), prior_rows.get(&id));
            }
        }
    }
}

#[test]
fn zeros_duplicates_equal_totals_and_last_retractions_keep_membership() {
    let mut node = SumGroupNode::<usize, usize>::new();
    assert_eq!(
        node.apply_batch([
            TupleChange::put(0, (7, 0)),
            TupleChange::put(1, (7, 4)),
            TupleChange::put(2, (7, -4)),
            TupleChange::put(3, (8, 0)),
        ]),
        vec![TupleChange::put(7, 0), TupleChange::put(8, 0)]
    );
    assert_eq!(
        node.apply(TupleChange::put(0, (7, 0))),
        vec![TupleChange::put(7, 0)]
    );
    assert_eq!(
        node.apply_batch([TupleChange::put(1, (7, -4)), TupleChange::put(2, (7, 4)),]),
        vec![TupleChange::put(7, 0)]
    );
    assert_eq!(
        node.apply(TupleChange::retract(3)),
        vec![TupleChange::retract(8)]
    );
    assert_eq!(
        node.apply_batch([TupleChange::retract(1), TupleChange::retract(2)]),
        vec![TupleChange::put(7, 0)]
    );
    assert_eq!(
        node.apply(TupleChange::retract(0)),
        vec![TupleChange::retract(7)]
    );
    assert!(node.apply(TupleChange::retract(0)).is_empty());
}

#[derive(Default)]
struct Faults {
    comparisons: Cell<usize>,
    clones: Cell<usize>,
    compare_failure: Cell<Option<usize>>,
    clone_failure: Cell<Option<usize>>,
}

#[derive(Debug)]
struct Key {
    value: usize,
    tag: usize,
    faults: Rc<Faults>,
}

impl std::fmt::Debug for Faults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Faults")
    }
}

impl Clone for Key {
    fn clone(&self) -> Self {
        let call = self.faults.clones.get() + 1;
        self.faults.clones.set(call);
        assert_ne!(self.faults.clone_failure.get(), Some(call), "clone failure");
        Self {
            value: self.value,
            tag: self.tag,
            faults: self.faults.clone(),
        }
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
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
        let call = self.faults.comparisons.get() + 1;
        self.faults.comparisons.set(call);
        assert_ne!(
            self.faults.compare_failure.get(),
            Some(call),
            "comparison failure"
        );
        self.value.cmp(&other.value)
    }
}

fn key(value: usize, tag: usize, faults: &Rc<Faults>) -> Key {
    Key {
        value,
        tag,
        faults: faults.clone(),
    }
}

#[test]
fn equal_key_representatives_survive_rounds_and_repeated_id_representatives_are_first() {
    let faults = Rc::new(Faults::default());
    let mut node = SumGroupNode::new();
    node.apply(TupleChange::put(
        key(0, 10, &faults),
        (key(7, 70, &faults), 5),
    ));
    let output = node.apply_batch([
        TupleChange::retract(key(0, 20, &faults)),
        TupleChange::put(key(1, 11, &faults), (key(7, 71, &faults), 9)),
    ]);
    assert_eq!(output[0].id.tag, 70);
    assert_eq!(output[0].value, Some(9));
    let output = node.apply_batch([
        TupleChange::put(key(4, 40, &faults), (key(8, 84, &faults), 1)),
        TupleChange::put(key(3, 30, &faults), (key(8, 83, &faults), 2)),
        TupleChange::put(key(3, 31, &faults), (key(8, 85, &faults), 3)),
    ]);
    assert_eq!(output[0].id.tag, 85);
    assert_eq!(output[0].value, Some(4));
    let GroupInputs::Tree(inputs) = &node.inputs else {
        panic!("generic IDs")
    };
    assert_eq!(inputs.keys().find(|id| id.value == 3).unwrap().tag, 30);
    assert_eq!(node.groups().find(|(k, _)| k.value == 7).unwrap().0.tag, 70);
    // Direct replacement keeps that representative too; output stays key ordered.
    let output = node.apply(TupleChange::put(
        key(1, 12, &faults),
        (key(6, 60, &faults), 3),
    ));
    assert_eq!(
        output.iter().map(|v| v.id.value).collect::<Vec<_>>(),
        [6, 7]
    );
}

#[test]
fn every_comparison_and_clone_panic_preserves_input_owners_groups_and_recovery() {
    for batched in [false, true] {
        let faults = Rc::new(Faults::default());
        let fresh = || {
            let mut node = SumGroupNode::new();
            node.apply_batch((0..6).map(|id| {
                TupleChange::put(
                    key(id, id, &faults),
                    (key(id % 2, id % 2, &faults), id as i64),
                )
            }));
            node
        };
        let change = || TupleChange::put(key(0, 100, &faults), (key(3, 30, &faults), 99));
        let run = |node: &mut SumGroupNode<Key, Key>| {
            if batched {
                node.apply_batch([
                    change(),
                    TupleChange::retract(key(2, 200, &faults)),
                    TupleChange::put(key(7, 700, &faults), (key(4, 40, &faults), 77)),
                ])
            } else {
                node.apply(change())
            }
        };
        let mut node = fresh();
        faults.comparisons.set(0);
        faults.clones.set(0);
        let expected = run(&mut node);
        let counts = [faults.comparisons.get(), faults.clones.get()];
        let expected: Vec<_> = expected
            .into_iter()
            .map(|v| (v.id.value, v.id.tag, v.value))
            .collect();
        for (kind, count) in counts.into_iter().enumerate() {
            assert!(count > 0);
            for failure in 1..=count {
                let mut node = fresh();
                let held = node.clone();
                faults.comparisons.set(0);
                faults.clones.set(0);
                if kind == 0 {
                    faults.compare_failure.set(Some(failure));
                } else {
                    faults.clone_failure.set(Some(failure));
                }
                assert!(catch_unwind(AssertUnwindSafe(|| run(&mut node))).is_err());
                faults.compare_failure.set(None);
                faults.clone_failure.set(None);
                let groups = |n: &SumGroupNode<Key, Key>| {
                    n.groups()
                        .map(|(k, v)| (k.value, k.tag, v))
                        .collect::<Vec<_>>()
                };
                assert_eq!(groups(&node), groups(&held));
                for id in 0..8 {
                    let captured = |n: &SumGroupNode<Key, Key>| {
                        n.input(&key(id, 0, &faults))
                            .map(|(k, v)| (k.value, k.tag, *v))
                    };
                    assert_eq!(captured(&node), captured(&held));
                    if let (Some(a), Some(b)) = (
                        node.input(&key(id, 0, &faults)),
                        held.input(&key(id, 0, &faults)),
                    ) {
                        assert!(std::ptr::eq(a, b));
                    }
                }
                assert_eq!(
                    run(&mut node)
                        .into_iter()
                        .map(|v| (v.id.value, v.id.tag, v.value))
                        .collect::<Vec<_>>(),
                    expected
                );
                node.apply_batch((0..8).map(|id| TupleChange::retract(key(id, id, &faults))));
                assert!(node.groups().next().is_none());
            }
        }
    }
}

#[test]
fn fixed_and_generic_captures_survive_nested_journals_full_installs_and_clear() {
    for fixed in [false, true] {
        let mut node = SumGroupNode::new();
        node.apply_batch([
            TupleChange::put(0usize, (0usize, 3)),
            TupleChange::put(255, (1, 0)),
        ]);
        if fixed {
            assert!(node.bind_fixed_inputs(256));
        }
        let original = node.clone();
        let outer = node.begin_checkpoint();
        node.apply(TupleChange::put(0, (1, 7)));
        let held = node.checkpoint();
        let inner = node.begin_checkpoint();
        node.apply_batch([TupleChange::put(0, (1, 9)), TupleChange::retract(255)]);
        node.commit_checkpoint(inner);
        assert_eq!(observed(&node), BTreeMap::from([(1, 9)]));
        node.restore(held);
        assert_eq!(observed(&node), BTreeMap::from([(1, 7)]));
        assert_eq!(node.input(&255), Some(&(1, 0)));
        node.clear();
        node.apply(TupleChange::put(9, (2, 11)));
        node.restore(outer);
        assert_eq!(observed(&node), observed(&original));
        assert!(std::ptr::eq(
            node.input(&0).unwrap(),
            original.input(&0).unwrap()
        ));
        assert_eq!(node.input(&255), original.input(&255));
        assert!(node.input(&9).is_none());
        if fixed {
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || node.apply(TupleChange::put(256, (2, 99)))
                ))
                .is_err()
            );
            assert_eq!(observed(&node), observed(&original));
            assert_eq!(node.input(&255), original.input(&255));
        }
    }
    let mut empty = SumGroupNode::<usize, usize>::new();
    assert!(empty.bind_fixed_inputs(0));
    assert!(!empty.bind_fixed_inputs(0));
    let mut large = SumGroupNode::new();
    large.apply(TupleChange::put(256usize, (0usize, 1)));
    assert!(!large.bind_fixed_inputs(256));
    assert!(!large.bind_fixed_inputs(257));
    let mut generic = SumGroupNode::new();
    generic.apply(TupleChange::put("x", (0usize, 2)));
    assert!(!generic.bind_fixed_inputs(1));
    assert_eq!(generic.input(&"x"), Some(&(0, 2)));
}

#[test]
fn graph_batch_mapping_is_final_only_and_zero_and_equal_sums_still_score() {
    let mapped = RefCell::new(Vec::new());
    let scored = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let groups = b
        .group_sum_batched(source.port(), |row| {
            mapped.borrow_mut().push(*row);
            *row
        })
        .unwrap();
    let data = b.dataset(&groups).unwrap();
    let terminal = b
        .score("sum", &groups, |row| {
            scored.borrow_mut().push(*row);
            SimpleScore::new(row.1)
        })
        .unwrap();
    let mut graph = b.build();
    let mut batch = StreamBatch::new();
    for (id, row) in [(4, (1, 5)), (3, (1, 90)), (9, (2, 10)), (3, (1, -5))] {
        batch.push(&source, TupleChange::put(id, row)).unwrap();
    }
    batch.push(&source, TupleChange::retract(9)).unwrap();
    {
        let mut tx = graph.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score_evaluations(), 1);
        assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
        tx.commit().unwrap();
    }
    assert_eq!(*mapped.borrow(), [(1, -5), (1, 5)]);
    assert_eq!(*scored.borrow(), [(1, 0)]);
    assert_eq!(
        graph.constraint_matches(&terminal).unwrap()[&1],
        ((1, 0), SimpleScore::ZERO)
    );
    assert_eq!(graph.dataset_rows(&data).unwrap()[&1], (1, 0));
    assert_eq!(graph.source_rows(&source).unwrap().len(), 2);
    let held = graph.source_rows(&source).unwrap().clone();
    mapped.borrow_mut().clear();
    scored.borrow_mut().clear();
    {
        let mut batch = StreamBatch::new();
        batch.push(&source, TupleChange::put(3, (1, 5))).unwrap();
        batch.push(&source, TupleChange::put(4, (1, -5))).unwrap();
        let mut tx = graph.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score_evaluations(), 1);
        tx.commit().unwrap();
    }
    assert_eq!(*scored.borrow(), [(1, 0)]);
    assert_eq!(held[&3], (1, -5));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(3)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(-5));
    tx.apply(&source, TupleChange::retract(4)).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    tx.commit().unwrap();
    assert!(graph.dataset_rows(&data).unwrap().is_empty());
    assert!(graph.constraint_matches(&terminal).unwrap().is_empty());
    graph.assert_consistent().unwrap();
}

#[test]
fn all_four_builders_preserve_immediate_or_final_publication_and_validate_ports() {
    for wrapping in [false, true] {
        for batched in [false, true] {
            let calls = Cell::new(0usize);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<String, (usize, i64)>();
            let mapping = |row: &(usize, i64)| *row;
            let groups = match (wrapping, batched) {
                (false, false) => b.group_sum(source.port(), mapping),
                (false, true) => b.group_sum_batched(source.port(), mapping),
                (true, false) => b.group_sum_wrapping(source.port(), mapping),
                (true, true) => b.group_sum_wrapping_batched(source.port(), mapping),
            }
            .unwrap();
            b.score("sum", &groups, |row| {
                calls.set(calls.get() + 1);
                SimpleScore::new(row.1)
            })
            .unwrap();
            let mut graph = b.build();
            {
                let mut batch = StreamBatch::new();
                batch
                    .push(&source, TupleChange::put("a".to_owned(), (0, 1)))
                    .unwrap();
                batch
                    .push(&source, TupleChange::put("b".to_owned(), (0, 2)))
                    .unwrap();
                let mut tx = graph.transaction();
                tx.apply_batch(batch).unwrap();
                assert_eq!(tx.score_evaluations(), if batched { 1 } else { 2 });
                tx.commit().unwrap();
            }
            assert_eq!(calls.get(), if batched { 1 } else { 2 });
            {
                let mut tx = graph.transaction();
                tx.apply(&source, TupleChange::put("a".to_owned(), (0, 5)))
                    .unwrap();
                assert_eq!(tx.score_evaluations(), 1);
                assert_eq!(tx.score().unwrap(), SimpleScore::new(7));
            }
            assert_eq!(graph.score(), SimpleScore::new(3));
            graph.assert_consistent().unwrap();
            let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
            assert!(matches!(
                foreign.group_sum(source.port(), mapping),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                foreign.group_sum_batched(source.port(), mapping),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                foreign.group_sum_wrapping(source.port(), mapping),
                Err(GraphError::ForeignPort)
            ));
            assert!(matches!(
                foreign.group_sum_wrapping_batched(source.port(), mapping),
                Err(GraphError::ForeignPort)
            ));
        }
    }
}

#[test]
fn graph_mapping_filter_score_and_structural_failures_restore_all_captures() {
    for batched in [false, true] {
        let failure = Cell::new(0usize);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, i64)>();
        let mapping = |row: &(usize, i64)| {
            assert_ne!(failure.get(), 1, "mapping failure");
            *row
        };
        let groups = if batched {
            b.group_sum_batched(source.port(), mapping)
        } else {
            b.group_sum(source.port(), mapping)
        }
        .unwrap();
        let data = b.dataset(&groups).unwrap();
        let filtered = b
            .filter(&groups, |row| {
                assert_ne!(failure.get(), 2, "filter failure");
                row.1 != 0
            })
            .unwrap();
        let terminal = b
            .score("sum", &filtered, |row| {
                assert_ne!(failure.get(), 3, "score failure");
                SimpleScore::new(row.1).with_structural(i64::from(failure.get() == 4))
            })
            .unwrap();
        let mut graph = b.build();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, (0, 5))).unwrap();
            tx.commit().unwrap();
        }
        let rows = graph.source_rows(&source).unwrap().clone();
        let data_before = graph.dataset_rows(&data).unwrap().clone();
        let matches_before = graph.constraint_matches(&terminal).unwrap().clone();
        for failed in 1..=4 {
            failure.set(failed);
            {
                let mut tx = graph.transaction();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut batch = StreamBatch::new();
                    batch.push(&source, TupleChange::put(0, (1, 9))).unwrap();
                    batch.push(&source, TupleChange::put(1, (1, 2))).unwrap();
                    tx.apply_batch(batch)
                }));
                if failed == 4 {
                    assert!(result.unwrap().is_err());
                } else {
                    assert!(result.is_err());
                }
                assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
            }
            failure.set(0);
            assert_eq!(graph.source_rows(&source).unwrap(), &rows);
            assert_eq!(graph.dataset_rows(&data).unwrap(), &data_before);
            assert_eq!(
                graph.constraint_matches(&terminal).unwrap(),
                &matches_before
            );
            assert_eq!(graph.score(), SimpleScore::new(5));
            graph.assert_consistent().unwrap();
        }
    }
}

#[test]
fn checked_graph_overflow_rolls_back_and_wrapping_graph_preserves_modular_total() {
    for wrapping in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, i64)>();
        let groups = if wrapping {
            b.group_sum_wrapping_batched(source.port(), |row| *row)
        } else {
            b.group_sum_batched(source.port(), |row| *row)
        }
        .unwrap();
        let data = b.dataset(&groups).unwrap();
        b.score("observe", &groups, |_| SimpleScore::ZERO).unwrap();
        let mut graph = b.build();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, (0, i64::MIN)))
                .unwrap();
            tx.commit().unwrap();
        }
        {
            let mut tx = graph.transaction();
            let result = catch_unwind(AssertUnwindSafe(|| {
                tx.apply(&source, TupleChange::put(0, (0, i64::MAX)))
            }));
            if wrapping {
                result.unwrap().unwrap();
                tx.commit().unwrap();
            } else {
                assert!(result.is_err());
                assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
            }
        }
        let expected = if wrapping { i64::MAX } else { i64::MIN };
        assert_eq!(graph.source_rows(&source).unwrap()[&0], (0, expected));
        assert_eq!(graph.dataset_rows(&data).unwrap()[&0], (0, expected));
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn saved_graphs_and_full_snapshots_restore_sums_after_later_trials_and_rebuilds() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, (usize, i64)>();
        let filtered = b.filter(source.port(), |row| row.1 >= 0).unwrap();
        let groups = b.group_sum_batched(&filtered, |row| *row).unwrap();
        let data = b.dataset(&groups).unwrap();
        let terminal = b
            .score("sum", &groups, |row| SimpleScore::new(row.1))
            .unwrap();
        let mut graph = b.build();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, (0, 1))).unwrap();
            tx.apply(&source, TupleChange::put(1, (1, -5))).unwrap();
            tx.commit().unwrap();
        }
        let original = graph.capture_full();
        let source_snapshot = graph.source_rows(&source).unwrap().clone();
        let data_snapshot = graph.dataset_rows(&data).unwrap().clone();
        let saved;
        {
            let mut tx = graph.transaction();
            let mut batch = StreamBatch::new();
            batch.push(&source, TupleChange::put(0, (2, 8))).unwrap();
            batch.push(&source, TupleChange::put(1, (2, 0))).unwrap();
            tx.apply_batch(batch).unwrap();
            tx.assert_consistent().unwrap();
            saved = tx.capture().unwrap();
        }
        for amount in [2, 3, 4] {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, (0, amount))).unwrap();
            tx.assert_consistent().unwrap();
        }
        assert_eq!(graph.source_rows(&source).unwrap(), &source_snapshot);
        graph.install(saved);
        assert_eq!(graph.score(), SimpleScore::new(8));
        assert_eq!(graph.dataset_rows(&data).unwrap()[&2], (2, 8));
        assert_eq!(
            graph.constraint_matches(&terminal).unwrap()[&2].1,
            SimpleScore::new(8)
        );
        graph.assert_consistent().unwrap();
        graph.install(original);
        assert_eq!(graph.source_rows(&source).unwrap(), &source_snapshot);
        assert_eq!(graph.dataset_rows(&data).unwrap(), &data_snapshot);
        assert_eq!(graph.score(), SimpleScore::new(1));
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn disabled_branches_reactivate_from_retained_source_rows_with_zero_memberships() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, (usize, i64)>();
        let groups = b.group_sum_batched(source.port(), |row| *row).unwrap();
        let terminal = b
            .score_weighted("sum", &groups, SimpleScore::ZERO, |row| row.1)
            .unwrap();
        let mut graph = b.build();
        for rows in [
            vec![(0, (0, 4)), (1, (0, -4))],
            vec![(0, (1, 7)), (2, (2, 0))],
        ] {
            let mut tx = graph.transaction();
            let mut batch = StreamBatch::new();
            for (id, row) in rows {
                batch.push(&source, TupleChange::put(id, row)).unwrap();
            }
            tx.apply_batch(batch).unwrap();
            assert_eq!(tx.score_evaluations(), 0);
            tx.commit().unwrap();
        }
        let expected = scan(
            &graph
                .source_rows(&source)
                .unwrap()
                .iter()
                .map(|(id, row)| (*id, *row))
                .collect(),
            false,
        );
        let weights = |amount| -> ConstraintWeightOverrides<SimpleScore> {
            [("sum".to_owned(), SimpleScore::new(amount))]
                .into_iter()
                .collect()
        };
        assert_eq!(
            graph.apply_weight_overrides(&weights(1)).unwrap(),
            expected.len() as u128
        );
        let actual: BTreeMap<_, _> = graph
            .constraint_matches(&terminal)
            .unwrap()
            .iter()
            .map(|(key, (row, _))| (*key, row.1))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.get(&2), Some(&0));
        assert_eq!(graph.source_rows(&source).unwrap().len(), 3);
        graph.assert_consistent().unwrap();
        graph.apply_weight_overrides(&weights(0)).unwrap();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::retract(1)).unwrap();
            tx.apply(&source, TupleChange::put(2, (2, 3))).unwrap();
            tx.commit().unwrap();
        }
        graph.apply_weight_overrides(&weights(2)).unwrap();
        assert_eq!(graph.score(), SimpleScore::new(20));
        assert_eq!(graph.source_rows(&source).unwrap().len(), 2);
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn real_planning_saves_the_best_group_state_and_batches_composite_moves() {
    use crate::search_scoring::SearchDirector;
    for count in [3usize, 257] {
        for wrapping in [false, true] {
            let calls = Cell::new(0usize);
            let problem = FiniteProblem::new(
                (0..count)
                    .map(|_| Variable::new(vec![-2i64, 1, 2]).with_initial(1))
                    .collect(),
            )
            .unwrap();
            let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
            let source = b.source::<usize, (usize, Option<i64>)>();
            let assigned = b.filter(source.port(), |row| row.1.is_some()).unwrap();
            let mapped = b.map(&assigned, |row| row.1.unwrap()).unwrap();
            let groups = if wrapping {
                b.group_sum_wrapping_batched(&mapped, |value| ((), *value))
            } else {
                b.group_sum_batched(&mapped, |value| ((), *value))
            }
            .unwrap();
            let data = b.dataset(&groups).unwrap();
            b.score("sum", &groups, |row| {
                calls.set(calls.get() + 1);
                SimpleScore::new(row.1)
            })
            .unwrap();
            let inspect = source.clone();
            let mut director =
                StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
                    .unwrap();
            calls.set(0);
            let selected = director
                .select_best(
                    [2, -2, 1].map(|value| Move::Change {
                        variable: 0,
                        value: Some(value),
                    }),
                    3,
                    ForagerConfig::<SimpleScore>::default(),
                    |_, _| true,
                    |_| false,
                    false,
                )
                .unwrap();
            assert_eq!(selected.evaluations, 3);
            assert_eq!(selected.constraint_evaluations, 3);
            assert_eq!(calls.get(), 3, "winner installation must not rescore");
            assert_eq!(director.assignment().values()[0], Some(2));
            assert_eq!(director.score(), SimpleScore::new(count as i64 + 1));
            assert_eq!(director.graph().source_rows(&inspect).unwrap().len(), count);
            assert_eq!(
                director.graph().dataset_rows(&data).unwrap()[&()].1,
                count as i64 + 1
            );
            let movement = Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: Some(-2),
                },
                Move::Change {
                    variable: 1,
                    value: Some(2),
                },
            ]);
            calls.set(0);
            {
                let trial = director.apply(&movement).unwrap();
                assert_eq!(trial.constraint_evaluations(), 1);
                assert_eq!(trial.score(), SimpleScore::new(count as i64 - 2));
            }
            assert_eq!(calls.get(), 1);
            assert_eq!(director.score(), SimpleScore::new(count as i64 + 1));
            director.assert_consistent().unwrap();
        }
    }
}

#[test]
fn graph_key_panics_before_and_after_sum_installation_restore_rows_and_matches() {
    for clone_panic in [false, true] {
        let faults = Rc::new(Faults::default());
        let calls = Cell::new(0usize);
        let fixture = || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<Key, (Key, i64)>();
            let groups = b
                .group_sum_batched(source.port(), |row| row.clone())
                .unwrap();
            let data = b.dataset(&groups).unwrap();
            let terminal = b
                .score("sum", &groups, |row| {
                    calls.set(calls.get() + 1);
                    SimpleScore::new(row.1)
                })
                .unwrap();
            let mut graph = b.build();
            {
                let mut tx = graph.transaction();
                for id in 0..4 {
                    tx.apply(
                        &source,
                        TupleChange::put(
                            key(id, id, &faults),
                            (key(id % 2, id % 2, &faults), id as i64),
                        ),
                    )
                    .unwrap();
                }
                tx.commit().unwrap();
            }
            (graph, source, data, terminal)
        };
        let changes = |source: &StreamSource<Key, (Key, i64)>| {
            let mut batch = StreamBatch::new();
            batch
                .push(
                    source,
                    TupleChange::put(key(0, 10, &faults), (key(3, 30, &faults), 9)),
                )
                .unwrap();
            batch
                .push(source, TupleChange::retract(key(2, 20, &faults)))
                .unwrap();
            batch
        };
        let (mut graph, source, _, _) = fixture();
        let batch = changes(&source);
        faults.comparisons.set(0);
        faults.clones.set(0);
        {
            let mut tx = graph.transaction();
            tx.apply_batch(batch).unwrap();
            tx.commit().unwrap();
        }
        let count = if clone_panic {
            faults.clones.get()
        } else {
            faults.comparisons.get()
        };
        assert!(count > 0);
        for failure in 1..=count {
            let (mut graph, source, data, terminal) = fixture();
            let rows = graph.source_rows(&source).unwrap().clone();
            let datasets = graph.dataset_rows(&data).unwrap().clone();
            let matches = graph.constraint_matches(&terminal).unwrap().clone();
            let batch = changes(&source);
            faults.comparisons.set(0);
            faults.clones.set(0);
            if clone_panic {
                faults.clone_failure.set(Some(failure));
            } else {
                faults.compare_failure.set(Some(failure));
            }
            {
                let mut tx = graph.transaction();
                assert!(catch_unwind(AssertUnwindSafe(|| tx.apply_batch(batch))).is_err());
                assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
                // Rollback itself must not invoke a key comparison or user Clone.
                faults
                    .compare_failure
                    .set(Some(faults.comparisons.get() + 1));
                faults.clone_failure.set(Some(faults.clones.get() + 1));
                calls.set(0);
            }
            faults.compare_failure.set(None);
            faults.clone_failure.set(None);
            assert_eq!(calls.get(), 0);
            assert_eq!(graph.source_rows(&source).unwrap(), &rows);
            assert_eq!(graph.dataset_rows(&data).unwrap(), &datasets);
            assert_eq!(graph.constraint_matches(&terminal).unwrap(), &matches);
            graph.assert_consistent().unwrap();
            let mut tx = graph.transaction();
            tx.apply_batch(changes(&source)).unwrap();
            tx.commit().unwrap();
            assert_eq!(graph.score(), SimpleScore::new(13));
        }
    }
}

#[test]
fn full_source_payloads_and_snapshot_owners_survive_and_release_in_normal_transactions() {
    #[derive(Clone)]
    struct Row {
        group: usize,
        value: i64,
        owner: Rc<()>,
    }
    let owner = Rc::new(());
    let weak = Rc::downgrade(&owner);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let groups = b
        .group_sum_batched(source.port(), |row| (row.group, row.value))
        .unwrap();
    b.score("sum", &groups, |row| SimpleScore::new(row.1))
        .unwrap();
    let mut graph = b.build();
    {
        let mut tx = graph.transaction();
        tx.apply(
            &source,
            TupleChange::put(
                0,
                Row {
                    group: 0,
                    value: 7,
                    owner,
                },
            ),
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let held = graph.source_rows(&source).unwrap().clone();
    let pointer = held[&0].owner.as_ref() as *const ();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(0)).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    }
    assert_eq!(
        graph.source_rows(&source).unwrap()[&0].owner.as_ref() as *const (),
        pointer
    );
    assert!(weak.upgrade().is_some());
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(0)).unwrap();
        tx.commit().unwrap();
    }
    assert!(weak.upgrade().is_some());
    drop(held);
    assert!(weak.upgrade().is_none());
    assert_eq!(graph.score(), SimpleScore::ZERO);
}

#[test]
fn checked_reconstruction_overflow_preserves_reachable_live_state() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let groups = b.group_sum_batched(source.port(), |row| *row).unwrap();
    let data = b.dataset(&groups).unwrap();
    b.score("observe", &groups, |_| SimpleScore::ZERO).unwrap();
    let mut graph = b.build();
    {
        let mut tx = graph.transaction();
        // This update order is valid; sorted-ID reconstruction overflows on ID 1.
        for (id, value) in [(0, i64::MAX), (2, -1), (1, 1)] {
            tx.apply(&source, TupleChange::put(id, (0, value))).unwrap();
        }
        tx.commit().unwrap();
    }
    let rows = graph.source_rows(&source).unwrap().clone();
    assert!(catch_unwind(AssertUnwindSafe(|| graph.assert_consistent())).is_err());
    assert_eq!(graph.source_rows(&source).unwrap(), &rows);
    assert_eq!(graph.dataset_rows(&data).unwrap()[&0], (0, i64::MAX));
    assert_eq!(graph.score(), SimpleScore::ZERO);
    // An ordinary later retraction recovers a reconstructible state.
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    graph.assert_consistent().unwrap();
}

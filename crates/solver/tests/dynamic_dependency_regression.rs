//! Original runtime dependency-membership regressions; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    sources: Vec<i32>,
    base: i64,
    emit: bool,
}
fn row(sources: &[i32], base: i64) -> Row {
    Row {
        sources: sources.to_vec(),
        base,
        emit: true,
    }
}
fn compute(
    _: &i32,
    row: &Row,
    inputs: &[DynamicDependencyInput<'_, i32, Row, i64>],
) -> Option<i64> {
    row.emit.then(|| {
        row.base
            + inputs
                .iter()
                .map(|input| {
                    input
                        .row
                        .map_or(-5, |r| 10 + r.base + input.derived.value.unwrap_or(0))
                })
                .sum::<i64>()
    })
}
fn contents(
    node: &DynamicDependencyNode<'_, i32, Row, i64>,
) -> BTreeMap<i32, (Row, DerivedValue<i64>)> {
    node.rows()
        .map(|(&k, (r, v))| (k, (r.clone(), v.clone())))
        .collect()
}
fn oracle(rows: &BTreeMap<i32, Row>) -> BTreeMap<i32, (Row, DerivedValue<i64>)> {
    fn derive(
        key: i32,
        rows: &BTreeMap<i32, Row>,
        visiting: &mut BTreeSet<i32>,
    ) -> DerivedValue<i64> {
        let Some(row) = rows.get(&key) else {
            return DerivedValue {
                value: None,
                inconsistent: false,
            };
        };
        if !visiting.insert(key) {
            return DerivedValue {
                value: None,
                inconsistent: true,
            };
        }
        let values: Vec<_> = row
            .sources
            .iter()
            .map(|&k| derive(k, rows, visiting))
            .collect();
        visiting.remove(&key);
        let inconsistent = values.iter().any(|v| v.inconsistent);
        let value = (!inconsistent && row.emit).then(|| {
            row.base
                + row
                    .sources
                    .iter()
                    .zip(values)
                    .map(|(k, v)| {
                        rows.get(k)
                            .map_or(-5, |r| 10 + r.base + v.value.unwrap_or(0))
                    })
                    .sum::<i64>()
        });
        DerivedValue {
            value,
            inconsistent,
        }
    }
    rows.iter()
        .map(|(&k, r)| (k, (r.clone(), derive(k, rows, &mut BTreeSet::new()))))
        .collect()
}
fn assert_reference(node: &DynamicDependencyNode<'_, i32, Row, i64>, rows: &BTreeMap<i32, Row>) {
    assert_eq!(contents(node), oracle(rows));
    let keys: BTreeSet<_> = rows
        .keys()
        .copied()
        .chain(rows.values().flat_map(|r| r.sources.iter().copied()))
        .collect();
    assert_eq!(node.keys(), keys.into_iter().collect::<Vec<_>>());
}
#[test]
fn source_presence_and_metadata_invalidate_direct_consumers_even_with_equal_none() {
    let calls = RefCell::new(vec![]);
    let mut node = DynamicDependencyNode::new(
        |r: &Row| r.sources.clone(),
        |k, r, inputs| {
            calls.borrow_mut().push(*k);
            compute(k, r, inputs)
        },
    );
    let mut silent = row(&[], 7);
    silent.emit = false;
    node.apply_batch([
        TupleChange::put(20, row(&[-10], 2)),
        TupleChange::put(30, row(&[20], 3)),
        TupleChange::put(40, row(&[], 4)),
    ])
    .unwrap();
    assert_eq!(contents(&node)[&20].1.value, Some(-3));
    calls.borrow_mut().clear();
    let changes = node.apply(TupleChange::put(-10, silent.clone())).unwrap();
    assert_eq!(*calls.borrow(), [-10, 20, 30]);
    assert_eq!(
        changes.iter().map(|c| c.id).collect::<Vec<_>>(),
        [-10, 20, 30]
    );
    assert_eq!(contents(&node)[&20].1.value, Some(19));
    assert_eq!(contents(&node)[&-10].1.value, None);
    silent.base = 8;
    calls.borrow_mut().clear();
    node.apply(TupleChange::put(-10, silent.clone())).unwrap();
    assert_eq!(*calls.borrow(), [-10, 20, 30]);
    assert_eq!(contents(&node)[&20].1.value, Some(20));
    calls.borrow_mut().clear();
    node.apply(TupleChange::retract(-10)).unwrap();
    assert_eq!(*calls.borrow(), [20, 30]);
    assert_eq!(node.keys(), [-10, 20, 30, 40]); // The absent source remains referenced.
    assert_eq!(contents(&node)[&20].1.value, Some(-3));
    node.apply(TupleChange::put(-10, silent)).unwrap();
    assert_eq!(contents(&node)[&20].1.value, Some(20));
    node.apply_batch([
        TupleChange::retract(-10),
        TupleChange::retract(20),
        TupleChange::retract(30),
    ])
    .unwrap();
    assert_eq!(node.keys(), [40]);
    node.apply(TupleChange::put(-10, row(&[], 9))).unwrap();
    assert_eq!(contents(&node)[&-10].1.value, Some(9));
}
#[test]
fn declaration_order_duplicates_and_final_batch_reclamation_are_observable() {
    let observed = RefCell::new(vec![]);
    let dependencies = Cell::new(0);
    let mut node = DynamicDependencyNode::new(
        |r: &Row| {
            dependencies.set(dependencies.get() + 1);
            r.sources.clone()
        },
        |k, r, inputs| {
            observed.borrow_mut().push((
                *k,
                inputs
                    .iter()
                    .map(|i| (*i.key, i.row.is_some()))
                    .collect::<Vec<_>>(),
            ));
            Some(
                r.base
                    + inputs
                        .iter()
                        .enumerate()
                        .map(|(n, i)| (n as i64 + 1) * i.derived.value.unwrap_or(0))
                        .sum::<i64>(),
            )
        },
    );
    assert!(
        node.apply_batch([
            TupleChange::put(100, row(&[200], 0)),
            TupleChange::retract(100),
            TupleChange::retract(300)
        ])
        .unwrap()
        .is_empty()
    );
    assert_eq!(dependencies.get(), 0);
    assert!(node.keys().is_empty());
    node.apply_batch([
        TupleChange::put(10, row(&[2, 1, 2, 0], 0)),
        TupleChange::put(1, row(&[], 3)),
        TupleChange::put(2, row(&[], 7)),
    ])
    .unwrap();
    assert_eq!(
        observed.borrow().last().unwrap(),
        &(10, vec![(2, true), (1, true), (2, true), (0, false)])
    );
    assert_eq!(contents(&node)[&10].1.value, Some(34));
    observed.borrow_mut().clear();
    node.apply_batch([
        TupleChange::retract(2),
        TupleChange::put(10, row(&[1, 2, 1], 0)),
        TupleChange::put(2, row(&[], 7)),
    ])
    .unwrap();
    assert_eq!(node.keys(), [1, 2, 10]);
    assert_eq!(
        observed.borrow().last().unwrap(),
        &(10, vec![(1, true), (2, true), (1, true)])
    );
    assert_eq!(contents(&node)[&10].1.value, Some(26));
    observed.borrow_mut().clear();
    node.apply(TupleChange::put(10, row(&[2, 1, 1], 0)))
        .unwrap(); // Same structural edges, different declaration order.
    assert_eq!(observed.borrow().len(), 1);
    assert_eq!(contents(&node)[&10].1.value, Some(22));
}
#[test]
fn reindexing_and_churn_preserve_unrelated_caches_and_bound_supplier_work() {
    let calls = RefCell::new(vec![]);
    let mut node = DynamicDependencyNode::new(
        |r: &Row| r.sources.clone(),
        |k, r, inputs| {
            calls.borrow_mut().push(*k);
            compute(k, r, inputs)
        },
    );
    node.apply_batch((0..1024).map(|k| TupleChange::put(k, row(&[], i64::from(k)))))
        .unwrap();
    let stable = contents(&node);
    for key in -100..0 {
        calls.borrow_mut().clear();
        node.apply(TupleChange::put(key, row(&[], 42))).unwrap(); // Every existing dense index shifts.
        assert_eq!(*calls.borrow(), [key]);
        assert_eq!(node.keys().len(), 1025);
        calls.borrow_mut().clear();
        node.apply(TupleChange::retract(key)).unwrap();
        assert!(calls.borrow().is_empty());
        assert_eq!(node.keys().len(), 1024);
    }
    assert_eq!(contents(&node), stable);
    // A changed source row dirties its direct consumer, but equal derived output
    // stops before the next level despite a simultaneous membership rebuild.
    let mut silent = row(&[0], 0);
    silent.emit = false;
    node.apply_batch([
        TupleChange::put(2000, silent),
        TupleChange::put(3000, row(&[2000], 0)),
    ])
    .unwrap();
    calls.borrow_mut().clear();
    node.apply_batch([
        TupleChange::put(0, row(&[], 9)),
        TupleChange::put(-1, row(&[], 2)),
    ])
    .unwrap();
    assert_eq!(calls.borrow().len(), 3);
    assert!(calls.borrow().contains(&0));
    assert!(calls.borrow().contains(&2000));
    assert!(calls.borrow().contains(&-1));
    assert!(!calls.borrow().contains(&3000));
}
#[test]
fn all_small_membership_and_cycle_transitions_match_independent_recursive_reference() {
    let keys = [-2, 5, 11];
    fn state(key: i32, choice: usize, keys: &[i32; 3]) -> Option<Row> {
        (choice != 0).then(|| {
            row(
                if choice == 1 {
                    &[]
                } else {
                    std::slice::from_ref(&keys[choice - 2])
                },
                i64::from(key) + 3,
            )
        })
    }
    for code in 0..125 {
        let mut rows = BTreeMap::new();
        let mut digits = code;
        for &key in &keys {
            if let Some(r) = state(key, digits % 5, &keys) {
                rows.insert(key, r);
            }
            digits /= 5;
        }
        let mut node = DynamicDependencyNode::new(|r: &Row| r.sources.clone(), compute);
        node.apply_batch(rows.iter().map(|(&k, r)| TupleChange::put(k, r.clone())))
            .unwrap();
        assert_reference(&node, &rows);
        for &key in &keys {
            for choice in 0..5 {
                let original = rows.get(&key).cloned();
                let replacement = state(key, choice, &keys);
                let mut next = rows.clone();
                if let Some(r) = &replacement {
                    next.insert(key, r.clone());
                } else {
                    next.remove(&key);
                }
                let before = contents(&node);
                let changes = node
                    .apply(TupleChange {
                        id: key,
                        value: replacement,
                    })
                    .unwrap();
                assert_reference(&node, &next);
                // Applying the emitted delta must exactly reconstruct final rows.
                let mut replayed = before;
                for change in changes {
                    if let Some(r) = change.value {
                        replayed.insert(change.id, r);
                    } else {
                        replayed.remove(&change.id);
                    }
                }
                assert_eq!(replayed, oracle(&next));
                node.apply(TupleChange {
                    id: key,
                    value: original,
                })
                .unwrap();
                assert_reference(&node, &rows);
            }
        }
    }
}
#[test]
fn cycle_membership_descendants_and_coordinated_rewires_survive_reclamation() {
    let calls = RefCell::new(vec![]);
    let mut node = DynamicDependencyNode::new(
        |r: &Row| r.sources.clone(),
        |k, r, i| {
            calls.borrow_mut().push(*k);
            compute(k, r, i)
        },
    );
    let mut rows = BTreeMap::from([(10, row(&[20], 1)), (20, row(&[], 2)), (30, row(&[10], 3))]);
    node.apply_batch(rows.iter().map(|(&k, r)| TupleChange::put(k, r.clone())))
        .unwrap();
    calls.borrow_mut().clear();
    rows.insert(10, row(&[], 1));
    rows.insert(20, row(&[10, -5], 2));
    rows.insert(-5, row(&[], 5));
    node.apply_batch([
        TupleChange::put(20, rows[&20].clone()),
        TupleChange::put(10, rows[&10].clone()),
        TupleChange::put(-5, rows[&-5].clone()),
    ])
    .unwrap();
    assert_reference(&node, &rows);
    assert!(contents(&node).values().all(|(_, v)| !v.inconsistent));
    rows.insert(-5, row(&[20], 5));
    node.apply(TupleChange::put(-5, rows[&-5].clone())).unwrap();
    rows.insert(30, row(&[20], 3));
    node.apply(TupleChange::put(30, rows[&30].clone())).unwrap();
    assert_reference(&node, &rows);
    assert!(contents(&node)[&30].1.inconsistent);
    rows.remove(&-5);
    node.apply(TupleChange::retract(-5)).unwrap();
    assert_reference(&node, &rows);
    assert!(!contents(&node)[&30].1.inconsistent);
    rows.insert(-5, row(&[20], 5));
    node.apply(TupleChange::put(-5, rows[&-5].clone())).unwrap();
    assert_reference(&node, &rows);
    rows.remove(&-5);
    rows.remove(&20);
    rows.remove(&30);
    node.apply_batch([
        TupleChange::retract(-5),
        TupleChange::retract(20),
        TupleChange::retract(30),
    ])
    .unwrap();
    assert_reference(&node, &rows);
    assert_eq!(node.keys(), [10]);
}
#[test]
fn dependency_supplier_and_output_clone_panics_preserve_membership_and_values() {
    let failure = Cell::new(0);
    let mut node = DynamicDependencyNode::new(
        |r: &Row| {
            assert!(failure.get() != 1 || r.base != 99, "dependency panic");
            r.sources.clone()
        },
        |k, r, i| {
            assert!(failure.get() != 2 || *k != 20, "supplier panic");
            compute(k, r, i)
        },
    );
    node.apply_batch([
        TupleChange::put(10, row(&[], 1)),
        TupleChange::put(20, row(&[10], 2)),
    ])
    .unwrap();
    let before = contents(&node);
    let keys = node.keys().to_vec();
    for mode in [1, 2] {
        failure.set(mode);
        assert!(
            catch_unwind(AssertUnwindSafe(|| node.apply_batch([
                TupleChange::retract(10),
                TupleChange::put(-5, row(&[], 3)),
                TupleChange::put(20, row(&[-5], 99))
            ])))
            .is_err()
        );
        assert_eq!(contents(&node), before);
        assert_eq!(node.keys(), keys);
    }
    failure.set(0);
    node.apply_batch([
        TupleChange::retract(10),
        TupleChange::put(-5, row(&[], 3)),
        TupleChange::put(20, row(&[-5], 99)),
    ])
    .unwrap();
    assert_eq!(node.keys(), [-5, 20]);
    struct Fragile(Rc<Cell<bool>>);
    impl Clone for Fragile {
        fn clone(&self) -> Self {
            assert!(!self.0.get(), "output clone panic");
            Self(self.0.clone())
        }
    }
    let fail = Rc::new(Cell::new(false));
    let mut node = DynamicDependencyNode::new(|_: &Fragile| vec![], |_, _, _| Some(1));
    node.apply(TupleChange::put(10, Fragile(fail.clone())))
        .unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_batch([
            TupleChange::retract(10),
            TupleChange::put(-5, Fragile(fail.clone()))
        ])))
        .is_err()
    );
    assert_eq!(node.keys(), [10]);
    assert_eq!(node.rows().count(), 1);
    fail.set(false);
    node.apply(TupleChange::put(-5, Fragile(fail))).unwrap();
    assert_eq!(node.keys(), [-5, 10]);
}
fn batch(
    source: &StreamSource<i32, Row>,
    changes: impl IntoIterator<Item = TupleChange<i32, Row>>,
) -> StreamBatch {
    let mut result = StreamBatch::new();
    for change in changes {
        result.push(source, change).unwrap();
    }
    result
}
#[test]
fn graph_batches_checkpoints_rebuilds_and_failures_restore_dynamic_membership() {
    let panic_supplier = Cell::new(false);
    let mut builder =
        StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(ScoringArithmetic::Checked);
    let source = builder.source::<i32, Row>();
    let derived = builder
        .derive_dynamic_dependencies(
            source.port(),
            |r| r.sources.clone(),
            |k, r, i| {
                assert!(!panic_supplier.get() || *k != -5, "supplier panic");
                compute(k, r, i)
            },
        )
        .unwrap();
    let dataset = builder.dataset(&derived).unwrap();
    let constraint = builder
        .score("derived", &derived, |(_, v)| {
            SimpleScore::new(v.value.unwrap_or(0))
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply_batch(batch(
        &source,
        [
            TupleChange::put(10, row(&[], 1)),
            TupleChange::put(20, row(&[10], 2)),
        ],
    ))
    .unwrap();
    tx.commit().unwrap();
    let before = graph.dataset_rows(&dataset).unwrap().clone();
    let sources = graph.source_rows(&source).unwrap().clone();
    let matches = graph.constraint_matches(&constraint).unwrap().clone();
    let score = graph.score();
    {
        let mut tx = graph.transaction();
        tx.apply_batch(batch(
            &source,
            [
                TupleChange::put(10, row(&[20, -5], 1)),
                TupleChange::put(20, row(&[], 2)),
                TupleChange::put(-5, row(&[], 3)),
            ],
        ))
        .unwrap();
        assert!(
            tx.graph()
                .unwrap()
                .dataset_rows(&dataset)
                .unwrap()
                .values()
                .all(|(_, v)| !v.inconsistent)
        );
        assert_eq!(tx.checkpoint_count(), 4); // source, derivation, dataset and terminal
        tx.apply(&source, TupleChange::put(20, row(&[10], 2)))
            .unwrap();
        assert!(
            tx.graph().unwrap().dataset_rows(&dataset).unwrap()[&10]
                .1
                .inconsistent
        );
        tx.apply(&source, TupleChange::retract(20)).unwrap();
        assert!(
            !tx.graph().unwrap().dataset_rows(&dataset).unwrap()[&10]
                .1
                .inconsistent
        );
    }
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
    assert_eq!(graph.score(), score);
    for panic in [false, true] {
        panic_supplier.set(panic);
        let mut tx = graph.transaction();
        let changes = [
            TupleChange::retract(10),
            TupleChange::retract(20),
            TupleChange::put(-5, row(&[], i64::MAX)),
            TupleChange::put(1, row(&[], 1)),
        ];
        if panic {
            assert!(
                catch_unwind(AssertUnwindSafe(|| tx.apply_batch(batch(&source, changes)))).is_err()
            );
        } else {
            assert!(tx.apply_batch(batch(&source, changes)).is_err()); // terminal total overflow after derivation succeeds
        }
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(graph.dataset_rows(&dataset).unwrap(), &before);
        assert_eq!(graph.source_rows(&source).unwrap(), &sources);
        assert_eq!(graph.constraint_matches(&constraint).unwrap(), &matches);
        assert_eq!(graph.score(), score);
    }
    panic_supplier.set(false);
    graph.assert_consistent().unwrap();
    let mut tx = graph.transaction();
    tx.apply_batch(batch(
        &source,
        [
            TupleChange::retract(10),
            TupleChange::retract(20),
            TupleChange::put(-5, row(&[], 3)),
        ],
    ))
    .unwrap();
    tx.commit().unwrap();
    graph.assert_consistent().unwrap();
    assert_eq!(graph.dataset_rows(&dataset).unwrap().len(), 1);
    assert_eq!(graph.score(), SimpleScore::new(3));
}

#[test]
fn pruned_branch_reactivation_rebuilds_only_current_membership_and_rolls_back() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = b.source::<i32, Row>();
    let gate = b.source::<usize, ()>();
    let derived = b
        .derive_dynamic_dependencies(
            source.port(),
            |r| r.sources.clone(),
            |k, r, i| {
                calls.set(calls.get() + 1);
                compute(k, r, i)
            },
        )
        .unwrap();
    let joined = b.join_equal(&derived, gate.port(), |_| (), |_| ()).unwrap();
    b.score("enabled", &joined, |((_, v), _)| {
        SimpleScore::new(v.value.unwrap_or(0))
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply_batch(batch(
        &source,
        [
            TupleChange::put(10, row(&[20], 1)),
            TupleChange::put(20, row(&[], 2)),
        ],
    ))
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 0);
    let mut tx = graph.transaction();
    tx.apply(&gate, TupleChange::put(0, ())).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(17));
    let mut tx = graph.transaction();
    tx.apply(&gate, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    let previous = calls.get();
    let mut tx = graph.transaction();
    tx.apply_batch(batch(
        &source,
        [
            TupleChange::retract(10),
            TupleChange::retract(20),
            TupleChange::put(-5, row(&[-9], 7)),
        ],
    ))
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), previous);
    {
        let mut tx = graph.transaction();
        tx.apply(&gate, TupleChange::put(0, ())).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(2));
    }
    assert_eq!(graph.score(), SimpleScore::ZERO);
    assert!(!graph.activity().get(&derived).unwrap().active);
    let mut tx = graph.transaction();
    tx.apply(&gate, TupleChange::put(0, ())).unwrap();
    tx.apply(&source, TupleChange::put(-9, row(&[], 3)))
        .unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(26));
    tx.commit().unwrap();
    graph.assert_consistent().unwrap();
    assert_eq!(graph.source_rows(&source).unwrap().len(), 2);
}

#[test]
fn shared_source_batch_does_not_clone_quadratically_growing_consumer_sets() {
    #[derive(Debug)]
    struct Key {
        id: usize,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                clones: self.clones.clone(),
            }
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.id.cmp(&other.id)
        }
    }
    let clones = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let key = |id| Key {
        id,
        clones: clones.clone(),
    };
    let count = 2048;
    let mut node = DynamicDependencyNode::new(
        |_: &()| vec![key(0)],
        |_, _, inputs| {
            calls.set(calls.get() + 1);
            assert_eq!(inputs.len(), 1);
            assert!(inputs[0].row.is_none());
            Some(1)
        },
    );
    node.apply_batch((1..=count).map(|id| TupleChange::put(key(id), ())))
        .unwrap();
    assert_eq!(node.keys().len(), count + 1);
    assert_eq!(calls.get(), count);
    // Persistent tree paths and final topology construction may clone keys, but
    // cloning a growing 2,048-entry consumer set on every edit exceeds this bound.
    assert!(
        clones.get() < count * 256,
        "fan-out insertion cloned {} keys",
        clones.get()
    );
    let inserted_clones = clones.get();
    clones.set(0);
    calls.set(0);
    node.apply_batch((1..=count).map(|id| TupleChange::retract(key(id))))
        .unwrap();
    assert!(node.keys().is_empty());
    assert_eq!(node.rows().count(), 0);
    assert_eq!(calls.get(), 0);
    assert!(
        clones.get() < count * 256,
        "fan-out removal cloned {} keys",
        clones.get()
    );
    eprintln!(
        "fan-out {count}: insert clones={inserted_clones}, remove clones={}",
        clones.get()
    );
}

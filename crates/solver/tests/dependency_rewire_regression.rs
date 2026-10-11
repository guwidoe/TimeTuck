//! Original sparse-rewiring regressions; no upstream test-method credit.
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn edges(mask: usize) -> Vec<(usize, usize)> {
    (0..9)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| (bit / 3, bit % 3))
        .collect()
}
fn reference(edges: &[(usize, usize)]) -> [DerivedValue<i64>; 3] {
    let mut reach = [[false; 3]; 3];
    for &(i, j) in edges {
        reach[i][j] = true;
    }
    for k in 0..3 {
        for i in 0..3 {
            for j in 0..3 {
                reach[i][j] |= reach[i][k] && reach[k][j];
            }
        }
    }
    let mut result: [DerivedValue<i64>; 3] = std::array::from_fn(|i| DerivedValue {
        value: None,
        inconsistent: (0..3).any(|j| reach[i][j] && reach[j][j]),
    });
    for _ in 0..3 {
        for i in 0..3 {
            if result[i].inconsistent {
                continue;
            }
            result[i].value = edges
                .iter()
                .filter(|(a, _)| *a == i)
                .try_fold(i as i64 + 1, |sum, (_, j)| {
                    result[*j].value.map(|v| sum + v)
                });
        }
    }
    result
}
fn supply(key: &usize, inputs: &[(&usize, &DerivedValue<i64>)]) -> Option<i64> {
    inputs
        .iter()
        .try_fold(*key as i64 + 1, |sum, (_, v)| v.value.map(|v| sum + v))
}
#[test]
fn every_three_node_graph_pair_matches_independent_batch_oracle() {
    for before in 0..512 {
        let old_edges = edges(before);
        let mut topology = DependencyTopology::new(0..3).unwrap();
        topology.update(&[], &old_edges).unwrap();
        let cache = DependencyCache::new(topology, supply);
        let previous = reference(&old_edges);
        for after in 0..512 {
            let next_edges = edges(after);
            let mut trial = cache.clone();
            // Include remove/readd duplicates and edges that are absent in both
            // states: effective changes, not requested operations, drive updates.
            let mut removals = old_edges.clone();
            removals.push((0, 0));
            let mut additions = next_edges.clone();
            additions.extend(next_edges.first().copied());
            let changed = trial.update(&removals, &additions, &[], supply).unwrap();
            let expected = reference(&next_edges);
            for (i, expected) in expected.iter().enumerate() {
                assert_eq!(&trial.values()[&i], expected, "{before} -> {after}, {i}");
                assert_eq!(
                    trial
                        .topology()
                        .dependencies(&i)
                        .unwrap()
                        .into_iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    next_edges
                        .iter()
                        .filter_map(|&(a, b)| (a == i).then_some(b))
                        .collect::<Vec<_>>()
                );
            }
            assert_eq!(
                changed,
                (0..3)
                    .filter(|&i| previous[i] != expected[i])
                    .collect::<Vec<_>>()
            );
        }
        for (i, expected) in previous.iter().enumerate() {
            assert_eq!(&cache.values()[&i], expected);
        }
    }
}

#[derive(Debug)]
struct Key {
    id: usize,
    clones: Rc<Cell<usize>>,
    panic_at: Rc<Cell<Option<usize>>>,
}
impl Clone for Key {
    fn clone(&self) -> Self {
        let count = self.clones.get() + 1;
        self.clones.set(count);
        assert_ne!(self.panic_at.get(), Some(count), "key clone panic");
        Self {
            id: self.id,
            clones: self.clones.clone(),
            panic_at: self.panic_at.clone(),
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}
#[test]
fn sparse_edits_and_snapshots_do_not_clone_the_key_universe() {
    let clones = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(None));
    let key = |id| Key {
        id,
        clones: clones.clone(),
        panic_at: panic_at.clone(),
    };
    let mut topology = DependencyTopology::new((0..4096).map(key)).unwrap();
    topology.update(&[], &[(key(4095), key(0))]).unwrap();
    clones.set(0);
    let saved = topology.clone();
    topology
        .update(&[(key(4095), key(0))], &[(key(4095), key(1))])
        .unwrap();
    assert_eq!(
        clones.get(),
        0,
        "sparse topology edit cloned unrelated metadata"
    );
    assert_eq!(saved.dependencies(&key(4095)).unwrap()[0].id, 0);
    assert_eq!(topology.dependencies(&key(4095)).unwrap()[0].id, 1);
    let mut cache = DependencyCache::new(topology, |_, _| Some(1));
    clones.set(0);
    let saved = cache.clone();
    let mut calls = 0;
    assert!(
        cache
            .update(
                &[(key(4095), key(1))],
                &[(key(4095), key(0))],
                &[],
                |_, _| {
                    calls += 1;
                    Some(1)
                }
            )
            .unwrap()
            .is_empty()
    );
    assert_eq!(calls, 1);
    assert_eq!(
        clones.get(),
        0,
        "sparse cache edit cloned unrelated metadata"
    );
    assert_eq!(saved.topology().dependencies(&key(4095)).unwrap()[0].id, 1);
}

#[test]
fn component_rebuild_key_clone_panics_preserve_topology() {
    let clones = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(None));
    let key = |id| Key {
        id,
        clones: clones.clone(),
        panic_at: panic_at.clone(),
    };
    let mut topology = DependencyTopology::new((0..4).map(key)).unwrap();
    topology
        .update(&[], &[(key(1), key(0)), (key(2), key(1))])
        .unwrap();
    let saved = topology.clone();
    clones.set(0);
    // Cycle keys clone first, then component keys. Panic during that latter
    // stage must not install edges, flags or a partially rebuilt component map.
    panic_at.set(Some(4));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            topology.update(&[], &[(key(0), key(2))]).unwrap();
        }))
        .is_err()
    );
    panic_at.set(None);
    assert_eq!(topology, saved);
    assert_eq!(
        topology
            .update(&[], &[(key(0), key(2))])
            .unwrap()
            .iter()
            .map(|k| k.id)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn rewired_reverse_links_survive_equal_results_cycles_and_rollback() {
    let mut topology = DependencyTopology::new(0..6).unwrap();
    topology.update(&[], &[(3, 0), (4, 3), (5, 4)]).unwrap();
    let evaluate = |key: &usize, inputs: &[(&usize, &DerivedValue<i64>)]| {
        Some(
            inputs
                .first()
                .map_or(*key as i64, |(_, v)| v.value.unwrap()),
        )
    };
    let mut cache = DependencyCache::new(topology, evaluate);
    // Equal derived results must still install reverse links to the new source.
    assert!(
        cache
            .update(&[(3, 0)], &[(3, 1)], &[1], |k, inputs| {
                if *k == 1 {
                    Some(0)
                } else {
                    evaluate(k, inputs)
                }
            })
            .unwrap()
            .contains(&1)
    );
    let saved = cache.clone();
    let mut calls = vec![];
    cache
        .update(&[], &[], &[0], |k, inputs| {
            calls.push(*k);
            evaluate(k, inputs)
        })
        .unwrap();
    assert_eq!(calls, [0]);
    assert_eq!(
        cache
            .update(&[], &[], &[1], |k, inputs| {
                if *k == 1 {
                    Some(9)
                } else {
                    evaluate(k, inputs)
                }
            })
            .unwrap(),
        [1, 3, 4, 5]
    );
    assert_eq!(cache.values()[&5].value, Some(9));
    assert_eq!(saved.values()[&5].value, Some(0));
    let stable = cache.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cache
                .update(&[(3, 1)], &[(3, 2)], &[], |_, _| -> Option<i64> {
                    panic!("supplier")
                })
                .unwrap();
        }))
        .is_err()
    );
    assert_eq!(cache.topology(), stable.topology());
    assert_eq!(cache.values(), stable.values());
    assert_eq!(
        cache.update(&[], &[(1, 5)], &[], evaluate).unwrap(),
        [1, 3, 4, 5]
    );
    assert!(cache.values()[&5].inconsistent);
    assert_eq!(
        cache.update(&[(3, 1)], &[(3, 2)], &[], evaluate).unwrap(),
        [1, 3, 4, 5]
    );
    assert_eq!(cache.values()[&1].value, Some(2));
    assert!(!cache.values()[&5].inconsistent);
}

#[test]
fn stream_rewiring_routes_new_roots_and_restores_previous_roots_on_undo() {
    let calls = Rc::new(RefCell::new(vec![]));
    let record = calls.clone();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (Option<usize>, i64)>();
    let derived = builder
        .derive_dependencies(
            source.port(),
            0..6,
            |row| row.0.into_iter().collect(),
            move |key, row, inputs| {
                record.borrow_mut().push(*key);
                Some(row.1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    let constraint = builder
        .score("derived", &derived, |(_, v)| {
            SimpleScore::new(v.value.unwrap_or(-100))
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    for (key, row) in [
        (0, (None, 10)),
        (1, (None, 10)),
        (2, (Some(0), 2)),
        (3, (Some(2), 3)),
    ] {
        tx.apply(&source, TupleChange::put(key, row)).unwrap();
    }
    tx.commit().unwrap();
    calls.borrow_mut().clear();
    let original = graph.constraint_matches(&constraint).unwrap().clone();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(2, (Some(1), 2)))
            .unwrap();
        assert_eq!(*calls.borrow(), [2]);
        calls.borrow_mut().clear();
        tx.apply(&source, TupleChange::put(0, (None, 20))).unwrap();
        assert_eq!(*calls.borrow(), [0]);
        calls.borrow_mut().clear();
        tx.apply(&source, TupleChange::put(1, (None, 20))).unwrap();
        assert_eq!(*calls.borrow(), [1, 2, 3]);
    }
    assert_eq!(graph.constraint_matches(&constraint).unwrap(), &original);
    calls.borrow_mut().clear();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (None, 20))).unwrap();
    assert_eq!(*calls.borrow(), [0, 2, 3]);
    tx.commit().unwrap();
    graph.assert_consistent().unwrap();
    let results: BTreeMap<_, _> = graph
        .constraint_matches(&constraint)
        .unwrap()
        .iter()
        .map(|(&k, v)| (k, v.0.1.value))
        .collect();
    assert_eq!(
        results,
        BTreeMap::from([(0, Some(20)), (1, Some(10)), (2, Some(22)), (3, Some(25))])
    );
}

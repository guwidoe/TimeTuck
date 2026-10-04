//! Original cache regressions; independent expectations earn no upstream credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn reference(
    facts: &[Option<i64>],
    edges: &[(usize, usize)],
) -> BTreeMap<usize, DerivedValue<i64>> {
    let n = facts.len();
    let mut reach = vec![vec![false; n]; n];
    for &(i, j) in edges {
        reach[i][j] = true;
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                reach[i][j] |= reach[i][k] && reach[k][j];
            }
        }
    }
    let bad: Vec<_> = (0..n)
        .map(|i| (0..n).any(|j| reach[i][j] && reach[j][j]))
        .collect();
    let mut out = BTreeMap::new();
    for (i, &is_bad) in bad.iter().enumerate() {
        if is_bad {
            out.insert(
                i,
                DerivedValue {
                    value: None,
                    inconsistent: true,
                },
            );
        }
    }
    while out.len() < n {
        let before = out.len();
        for (i, &fact) in facts.iter().enumerate() {
            if out.contains_key(&i) {
                continue;
            }
            let deps: Vec<_> = edges
                .iter()
                .filter_map(|&(a, b)| (a == i).then_some(b))
                .collect();
            if deps.iter().all(|k| out.contains_key(k)) {
                let value = fact.and_then(|base| {
                    deps.iter()
                        .try_fold(base, |sum, k| out[k].value.map(|v| sum + v))
                });
                out.insert(
                    i,
                    DerivedValue {
                        value,
                        inconsistent: false,
                    },
                );
            }
        }
        assert!(out.len() > before);
    }
    out
}
fn supplier(
    k: &usize,
    inputs: &[(&usize, &DerivedValue<i64>)],
    facts: &[Option<i64>],
) -> Option<i64> {
    facts[*k].and_then(|base| {
        inputs
            .iter()
            .try_fold(base, |sum, (_, v)| v.value.map(|v| sum + v))
    })
}
#[test]
fn every_small_graph_edge_and_fact_mutation_matches_independent_reachability() {
    for mask in 0..512 {
        let edges: Vec<_> = (0..9)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| (i / 3, i % 3))
            .collect();
        let facts = [Some(1), None, Some(3)];
        let mut topology = DependencyTopology::new(0..3).unwrap();
        topology.update(&[], &edges).unwrap();
        let original = DependencyCache::new(topology, |k, ins| supplier(k, ins, &facts));
        let expected = reference(&facts, &edges);
        assert_eq!(
            original
                .values()
                .iter()
                .map(|(&k, v)| (k, v.clone()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        for bit in 0..9 {
            let edge = (bit / 3, bit % 3);
            let mut changed_edges = edges.clone();
            let (removed, added) = if mask & (1 << bit) == 0 {
                changed_edges.push(edge);
                (vec![], vec![edge])
            } else {
                changed_edges.retain(|&e| e != edge);
                (vec![edge], vec![])
            };
            let mut trial = original.clone();
            let actual = trial
                .update(&removed, &added, &[], |k, ins| supplier(k, ins, &facts))
                .unwrap();
            let next = reference(&facts, &changed_edges);
            assert_eq!(
                trial
                    .values()
                    .iter()
                    .map(|(&k, v)| (k, v.clone()))
                    .collect::<BTreeMap<_, _>>(),
                next
            );
            assert_eq!(
                actual,
                (0..3)
                    .filter(|k| expected[k] != next[k])
                    .collect::<Vec<_>>()
            );
        }
        for dirty in 0..3 {
            let mut facts = facts;
            facts[dirty] = Some(10);
            let mut trial = original.clone();
            let changed = trial
                .update(&[], &[], &[dirty], |k, ins| supplier(k, ins, &facts))
                .unwrap();
            let next = reference(&facts, &edges);
            assert_eq!(
                trial
                    .values()
                    .iter()
                    .map(|(&k, v)| (k, v.clone()))
                    .collect::<BTreeMap<_, _>>(),
                next
            );
            assert_eq!(
                changed,
                (0..3)
                    .filter(|k| expected[k] != next[k])
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(
            original
                .values()
                .iter()
                .map(|(&k, v)| (k, v.clone()))
                .collect::<BTreeMap<_, _>>(),
            expected
        );
    }
}
#[test]
fn equal_values_cut_propagation_and_edge_identity_still_invalidates() {
    let mut topology = DependencyTopology::new(0..4).unwrap();
    topology.update(&[], &[(1, 0), (2, 1)]).unwrap();
    let compute = |k: &usize, ins: &[(&usize, &DerivedValue<i64>)], base| match k {
        0 => Some(base),
        1 => ins[0].1.value.map(|v| v % 2),
        2 => ins[0].1.value.map(|v| v + 1),
        _ => Some(7),
    };
    let mut cache = DependencyCache::new(topology, |k, ins| compute(k, ins, 2));
    let saved = cache.clone();
    let mut calls = vec![];
    assert_eq!(
        cache
            .update(&[], &[], &[0, 0], |k, ins| {
                calls.push(*k);
                compute(k, ins, 4)
            })
            .unwrap(),
        [0]
    );
    assert_eq!(calls, [0, 1]);
    assert_eq!(saved.values()[&0].value, Some(2));
    calls.clear();
    assert!(
        cache
            .update(&[], &[], &[0], |k, ins| {
                calls.push(*k);
                compute(k, ins, 4)
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(calls, [0]);
    assert!(
        cache
            .update(&[(1, 0)], &[(1, 0), (1, 0)], &[], |_, _| panic!(
                "no-op batch"
            ))
            .unwrap()
            .is_empty()
    );
    // Same input value, different input identity: callback must still run.
    cache.update(&[], &[], &[3], |_, _| Some(4)).unwrap();
    assert_eq!(
        cache
            .update(&[(1, 0)], &[(1, 3)], &[], |k, ins| if *k == 1 {
                Some(*ins[0].0 as i64)
            } else {
                ins[0].1.value.map(|v| v + 1)
            })
            .unwrap(),
        [1, 2]
    );
    assert_eq!(cache.values()[&2].value, Some(4));
}
#[test]
fn invalid_metadata_and_callback_panic_leave_both_plan_and_values_untouched() {
    let mut topology = DependencyTopology::new(0..3).unwrap();
    topology.update(&[], &[(1, 0), (2, 1)]).unwrap();
    let mut cache = DependencyCache::new(topology, |_, ins| {
        Some(ins.first().map_or(1, |(_, v)| v.value.unwrap() + 1))
    });
    let old_values = cache.values().clone();
    let old_plan = cache.topology().clone();
    assert_eq!(
        cache.update(&[], &[(0, 1)], &[99], |_, _| panic!()),
        Err(DependencyError::UnknownNode(99))
    );
    assert!(
        cache
            .update(&[(1, 0)], &[(0, 99)], &[0], |_, _| panic!())
            .is_err()
    );
    assert_eq!(cache.topology(), &old_plan);
    assert_eq!(cache.values(), &old_values);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cache
                .update(&[(2, 1)], &[(2, 0)], &[0], |k, _| {
                    if *k == 1 {
                        panic!("supplier");
                    }
                    Some(8)
                })
                .unwrap();
        }))
        .is_err()
    );
    assert_eq!(cache.topology(), &old_plan);
    assert_eq!(cache.values(), &old_values);
    assert_eq!(
        cache
            .update(&[], &[(0, 1)], &[], |_, _| panic!("cyclic or downstream"))
            .unwrap(),
        [0, 1, 2]
    );
    assert!(cache.values().iter().all(|(_, v)| v.inconsistent));
    cache
        .update(&[(0, 1)], &[], &[], |_, ins| {
            Some(ins.first().map_or(1, |(_, v)| v.value.unwrap() + 1))
        })
        .unwrap();
    assert_eq!(cache.values(), &old_values);
}
#[test]
fn snapshots_do_not_clone_values_and_local_updates_do_not_scan_the_universe() {
    #[derive(Eq, PartialEq)]
    struct Value(i64);
    #[derive(Clone)]
    struct Key(usize, Rc<Cell<usize>>);
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
            self.1.set(self.1.get() + 1);
            self.0.cmp(&r.0)
        }
    }
    let comparisons = Rc::new(Cell::new(0));
    let keys = (0..10_000).map(|i| Key(i, comparisons.clone()));
    let topology = DependencyTopology::new(keys).unwrap_or_else(|_| panic!());
    let mut cache = DependencyCache::new(topology, |k, _| Some(Value(k.0 as i64)));
    let saved = cache.clone();
    comparisons.set(0);
    let mut calls = 0;
    cache
        .update(&[], &[], &[Key(123, comparisons.clone())], |_, _| {
            calls += 1;
            Some(Value(-1))
        })
        .unwrap_or_else(|_| panic!());
    assert_eq!(calls, 1);
    assert!(comparisons.get() < 500, "{} comparisons", comparisons.get());
    assert_eq!(
        saved.values()[&Key(123, comparisons.clone())]
            .value
            .as_ref()
            .unwrap()
            .0,
        123
    );
    assert_eq!(
        cache.values()[&Key(123, comparisons.clone())]
            .value
            .as_ref()
            .unwrap()
            .0,
        -1
    );
}
#[test]
fn equality_panic_after_staged_changes_preserves_the_original_snapshot() {
    #[derive(Debug)]
    struct Value(i64);
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            assert_ne!(other.0, 99, "equality panic");
            self.0 == other.0
        }
    }
    impl Eq for Value {}
    let mut topology = DependencyTopology::new(0..3).unwrap();
    topology.update(&[], &[(1, 0), (2, 1)]).unwrap();
    let mut cache = DependencyCache::new(topology, |k, _| Some(Value(*k as i64)));
    let saved = cache.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cache
                .update(&[(2, 1)], &[(2, 0)], &[0], |k, _| {
                    Some(Value(if *k == 1 { 99 } else { 10 }))
                })
                .unwrap();
        }))
        .is_err()
    );
    assert_eq!(cache.values(), saved.values());
    assert_eq!(cache.topology(), saved.topology());
}

#[test]
fn wide_fanout_schedules_without_cloning_keys_and_late_clone_panic_is_atomic() {
    use std::cmp::Ordering;
    struct Key {
        name: String,
        clones: Rc<Cell<usize>>,
        panic_at: Rc<Cell<Option<usize>>>,
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            let n = self.clones.get() + 1;
            self.clones.set(n);
            assert_ne!(self.panic_at.get(), Some(n), "key clone");
            Self {
                name: self.name.clone(),
                clones: self.clones.clone(),
                panic_at: self.panic_at.clone(),
            }
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
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
            self.name.cmp(&other.name)
        }
    }
    let clones = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(None));
    let key = |name: String| Key {
        name,
        clones: clones.clone(),
        panic_at: panic_at.clone(),
    };
    let root = key("root".into());
    let mut topology = DependencyTopology::new(
        (0..1024)
            .rev()
            .map(|i| key(format!("leaf-{i:04}")))
            .chain([root.clone()]),
    )
    .unwrap_or_else(|_| panic!("topology"));
    let edges: Vec<_> = (0..1024)
        .map(|i| (key(format!("leaf-{i:04}")), root.clone()))
        .collect();
    topology
        .update(&[], &edges)
        .unwrap_or_else(|_| panic!("edges"));
    let mut cache = DependencyCache::new(topology, |k, inputs| {
        if k.name == "root" {
            assert!(inputs.is_empty());
            Some(0)
        } else {
            assert_eq!(inputs[0].0.name, "root");
            Some(5)
        }
    });
    clones.set(0);
    let mut calls = vec![];
    let changed = cache
        .update(&[], &[], std::slice::from_ref(&root), |k, inputs| {
            calls.push(k.name.clone());
            if k.name == "root" {
                Some(1)
            } else {
                assert_eq!(inputs.len(), 1);
                assert_eq!(inputs[0].0.name, "root");
                assert_eq!(inputs[0].1.value, Some(1));
                Some(5)
            }
        })
        .unwrap_or_else(|_| panic!("update"));
    assert_eq!(
        changed.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(),
        ["root"]
    );
    assert_eq!(calls[0], "root");
    assert_eq!(
        calls[1..],
        (0..1024)
            .map(|i| format!("leaf-{i:04}"))
            .collect::<Vec<_>>()
    );
    assert!(clones.get() < 10, "scheduling cloned {} keys", clones.get());
    let before = cache.clone();
    clones.set(0);
    panic_at.set(Some(2));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cache
                .update(&[], &[], std::slice::from_ref(&root), |k, _| {
                    Some(if k.name == "root" { 2 } else { 5 })
                })
                .unwrap_or_else(|_| panic!("update"));
        }))
        .is_err()
    );
    panic_at.set(None);
    assert!(cache.values() == before.values());
    assert!(cache.topology() == before.topology());
    assert_eq!(cache.values()[&root].value, Some(1));
}

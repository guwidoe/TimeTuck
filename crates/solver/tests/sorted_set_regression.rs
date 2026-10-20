//! Original regressions for the proposed public native set API.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    ReversibleCollector, SimpleScore, SortedSetCollector, StreamGraphBuilder, TupleChange,
};

#[derive(Debug)]
struct Value {
    key: i32,
    payload: String,
    fail_cmp: Rc<Cell<bool>>,
    fail_clone: Rc<Cell<bool>>,
}
impl Value {
    fn new(key: i32, payload: &str) -> Self {
        Self {
            key,
            payload: payload.into(),
            fail_cmp: Rc::new(Cell::new(false)),
            fail_clone: Rc::new(Cell::new(false)),
        }
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert!(!self.fail_clone.get(), "clone failure");
        Self {
            key: self.key,
            payload: self.payload.clone(),
            fail_cmp: self.fail_cmp.clone(),
            fail_clone: self.fail_clone.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        assert!(
            !self.fail_cmp.get() && !other.fail_cmp.get(),
            "comparison failure"
        );
        self.key.cmp(&other.key)
    }
}

#[test]
fn noncopy_values_have_sorted_immutable_set_api() {
    let mut c = SortedSetCollector::default();
    assert!(c.finish().is_empty());
    for k in [9, -2, 4] {
        c.insert(&Value::new(k, "payload"));
    }
    let set = c.finish();
    assert_eq!(set.len(), 3);
    let mut iterator = set.iter();
    assert_eq!(iterator.size_hint(), (3, Some(3)));
    assert_eq!(iterator.len(), 3);
    assert_eq!(iterator.next().unwrap().key, -2);
    assert_eq!(iterator.len(), 2);
    assert_eq!(iterator.by_ref().map(|v| v.key).collect::<Vec<_>>(), [4, 9]);
    assert_eq!(iterator.len(), 0);
    assert!(iterator.next().is_none());
    assert!(iterator.next().is_none());
    assert_eq!(
        (&set).into_iter().map(|v| v.key).collect::<Vec<_>>(),
        [-2, 4, 9]
    );

    assert_eq!(set.iter().map(|v| v.key).collect::<Vec<_>>(), [-2, 4, 9]);
    assert_eq!(set.first().unwrap().key, -2);
    assert_eq!(set.last().unwrap().key, 9);
    assert!(set.contains(&Value::new(4, "lookup")));
    assert!(!set.contains(&Value::new(5, "lookup")));
    assert!(format!("{set:?}").contains("payload"));
}
#[test]
fn duplicate_classes_retain_first_payload_until_final_removal() {
    let mut c = SortedSetCollector::default();
    let a = Value::new(1, "first");
    let b = Value::new(1, "later");
    c.insert(&a);
    c.insert(&b);
    assert!(c.remove(&b));
    assert_eq!(c.finish().first().unwrap().payload, "first");
    assert!(c.replace(&a, &b));
    assert_eq!(c.finish().first().unwrap().payload, "later");
    c.insert(&a);
    assert!(c.replace(&b, &a));
    assert_eq!(c.finish().first().unwrap().payload, "later");
    assert!(c.remove(&a));
    assert!(c.remove(&a));
    assert!(c.finish().is_empty());
}
#[test]
fn membership_equality_hides_counts_and_representatives() {
    let mut a = SortedSetCollector::default();
    let mut b = SortedSetCollector::default();
    a.insert(&Value::new(3, "a"));
    a.insert(&Value::new(3, "a"));
    b.insert(&Value::new(3, "b"));
    assert_eq!(a.finish(), b.finish());
    b.insert(&Value::new(4, "b"));
    assert_ne!(a.finish(), b.finish());
}
#[test]
fn failed_comparison_and_clone_roll_back_even_after_staged_removal() {
    let mut c = SortedSetCollector::default();
    for k in 0..32 {
        c.insert(&Value::new(k, "original"));
    }
    let saved = c.finish();
    let old = Value::new(15, "lookup");
    let new = Value::new(90, "new");
    new.fail_cmp.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&old, &new))).is_err());
    new.fail_cmp.set(false);
    assert_eq!(c.finish(), saved);
    new.fail_clone.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&old, &new))).is_err());
    assert_eq!(c.finish(), saved);
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&new))).is_err());
    new.fail_clone.set(false);
    assert_eq!(c.finish(), saved);
    old.fail_cmp.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| c.remove(&old))).is_err());
    old.fail_cmp.set(false);
    assert_eq!(c.finish(), saved);
    assert!(!c.remove(&new));
    assert!(!c.replace(&new, &old));
    assert_eq!(c.finish(), saved);
}
#[test]
fn snapshots_and_collector_clones_share_values_without_cloning_payloads() {
    let value = Value::new(2, "stable");
    let mut c = SortedSetCollector::default();
    c.insert(&value);
    value.fail_clone.set(true);
    let snapshot = c.finish();
    let mut branch = c.clone();
    branch.insert(&value); // Duplicate count changes never clone V.
    branch.insert(&Value::new(1, "branch"));
    branch.remove(&value);
    branch.remove(&value);
    assert_eq!(snapshot.first().unwrap().payload, "stable");
    assert_eq!(c.finish(), snapshot);
    assert_eq!(branch.finish().first().unwrap().key, 1);
}
#[test]
fn deterministic_multiset_history_matches_independent_counts() {
    let mut c = SortedSetCollector::default();
    let mut model = BTreeMap::<i32, usize>::new();
    let mut seed = 282u64;
    for _ in 0..4000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let key = ((seed >> 32) % 41) as i32 - 20;
        if seed & 3 == 0 {
            let exists = model.contains_key(&key);
            assert_eq!(c.remove(&key), exists);
            if let Some(n) = model.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    model.remove(&key);
                }
            }
        } else if seed & 3 == 1 {
            let new = key + 1;
            let exists = model.contains_key(&key);
            assert_eq!(c.replace(&key, &new), exists);
            if exists {
                let n = model.get_mut(&key).unwrap();
                *n -= 1;
                if *n == 0 {
                    model.remove(&key);
                }
                *model.entry(new).or_default() += 1;
            }
        } else {
            c.insert(&key);
            *model.entry(key).or_default() += 1;
        }
        assert_eq!(
            c.finish().iter().copied().collect::<Vec<_>>(),
            model.keys().copied().collect::<Vec<_>>()
        );
    }
    for (key, n) in model {
        for _ in 0..n {
            assert!(c.remove(&key));
        }
        assert!(!c.remove(&key));
    }
    assert!(c.finish().is_empty());
}
#[test]
fn real_graph_commit_and_rollback_preserve_counted_membership() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, String>();
    let out = b
        .group_collect_with::<_, _, _, _, SortedSetCollector<String>>(source.port(), |v| {
            ((), v.clone())
        })
        .unwrap();
    b.score("distinct", &out, |(_, set)| {
        SimpleScore::new(set.len() as i64)
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, "a".into())).unwrap();
    tx.apply(&source, TupleChange::put(1, "a".into())).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(1));
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, "b".into())).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(2));
    }
    assert_eq!(graph.score(), SimpleScore::new(1));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(1));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(1, "b".into())).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(1));
    graph.assert_consistent().unwrap();
}

//! Original regression controls; no upstream-method credit.
use super::*;
use std::{cell::Cell, cmp::Ordering, collections::BTreeMap, rc::Rc};

#[derive(Clone, Debug)]
struct Key(i32, i32);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
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
        self.0.cmp(&other.0)
    }
}

#[test]
fn immediate_replacements_retain_emissions_extrema_and_singleton_representatives() {
    let mut node = GroupNode::<i32, Key>::new();
    node.apply(TupleChange::put(1, (Key(1, 10), i64::MAX)));
    node.apply(TupleChange::put(2, (Key(1, 20), 1)));
    let output = node.apply(TupleChange::put(1, (Key(1, 30), -2)));
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].id.1, 10);
    assert_eq!(node.groups().next().unwrap().0.1, 10);
    assert_eq!(
        output[0].value.unwrap(),
        [-2, 1].into_iter().collect::<IntCollector>().summary()
    );
    let output = node.apply(TupleChange::put(1, (Key(1, 40), -2)));
    assert_eq!(output.len(), 1, "equal summaries still emit");
    assert_eq!(output[0].id.1, 30, "emission uses the old capture key");
    assert_eq!(node.groups().next().unwrap().0.1, 10);
    let moved = node.apply(TupleChange::put(2, (Key(2, 50), i64::MIN)));
    assert_eq!(moved.iter().map(|c| c.id.0).collect::<Vec<_>>(), [1, 2]);
    node.apply(TupleChange::put(1, (Key(1, 60), 8)));
    assert_eq!(node.groups().next().unwrap().0.1, 60);
    assert!(node.apply(TupleChange::retract(99)).is_empty());
}

#[test]
fn nested_trials_full_snapshots_and_bound_inputs_recover_complete_integer_multisets() {
    for bound in [false, true] {
        let mut node = GroupNode::<usize, i32>::new();
        for i in 0..8 {
            node.apply(TupleChange::put(i, ((i % 2) as i32, i as i64)));
        }
        if bound {
            assert!(node.bind_fixed_inputs(8));
        }
        let original: Vec<_> = node.groups().map(|(&k, v)| (k, v)).collect();
        let outer = node.begin_journal_checkpoint();
        node.apply(TupleChange::put(0, (0, i64::MAX)));
        node.apply(TupleChange::put(1, (0, -1)));
        let saved = node.checkpoint();
        let middle: Vec<_> = node.groups().map(|(&k, v)| (k, v)).collect();
        let inner = node.begin_journal_checkpoint();
        node.apply(TupleChange::put(0, (5, -9)));
        node.apply(TupleChange::retract(7));
        node.commit_journal_checkpoint(inner);
        node.restore_checkpoint(saved);
        assert_eq!(
            node.groups().map(|(&k, v)| (k, v)).collect::<Vec<_>>(),
            middle
        );
        node.clear();
        node.restore_checkpoint(outer);
        assert_eq!(
            node.groups().map(|(&k, v)| (k, v)).collect::<Vec<_>>(),
            original
        );
        node.apply(TupleChange::put(0, (0, 0)));
        assert_eq!(
            node.groups().map(|(&k, v)| (k, v)).collect::<Vec<_>>(),
            original
        );
    }
}

#[test]
fn immediate_mixed_transfers_match_independent_full_recomputation_after_each_change() {
    let mut inputs = BTreeMap::new();
    let mut node = GroupNode::new();
    let mut state = 17u64;
    for step in 0..1000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (state >> 32) as usize % 19;
        let new = if state.is_multiple_of(7) {
            None
        } else {
            Some(((state >> 40) as i32 % 5, (state >> 48) as i64 % 13 - 6))
        };
        let old = inputs.get(&id).copied();
        if let Some(new) = new {
            inputs.insert(id, new);
        } else {
            inputs.remove(&id);
        }
        let output = node.apply(TupleChange { id, value: new });
        let mut expected = BTreeMap::<i32, Vec<i64>>::new();
        for &(key, value) in inputs.values() {
            expected.entry(key).or_default().push(value);
        }
        let expected: BTreeMap<_, _> = expected
            .into_iter()
            .map(|(key, values)| {
                let mut distinct = values.clone();
                distinct.sort_unstable();
                distinct.dedup();
                (
                    key,
                    IntSummary {
                        count: values.len(),
                        distinct_count: distinct.len(),
                        sum: values.iter().fold(0i64, |s, &v| s.wrapping_add(v)),
                        min: values.iter().min().copied(),
                        max: values.iter().max().copied(),
                    },
                )
            })
            .collect();
        assert_eq!(
            node.groups()
                .map(|(&k, v)| (k, v))
                .collect::<BTreeMap<_, _>>(),
            expected,
            "step {step}"
        );
        let mut keys = Vec::new();
        if let Some((key, _)) = old {
            keys.push(key);
        }
        if let Some((key, _)) = new
            && !keys.contains(&key)
        {
            keys.push(key);
        }
        assert_eq!(output.iter().map(|c| c.id).collect::<Vec<_>>(), keys);
        for change in output {
            assert_eq!(change.value, expected.get(&change.id).copied());
        }
    }
}

#[test]
fn graph_trial_preserves_every_immediate_emission_and_recovers_after_sink_failure() {
    let fail = Rc::new(Cell::new(false));
    let seen = Rc::new(Cell::new(0));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, (i32, i64)>();
    let groups = builder.group_collect(source.port(), |&row| row).unwrap();
    let dataset = builder.dataset(&groups).unwrap();
    builder
        .score("sum", &groups, {
            let fail = fail.clone();
            let seen = seen.clone();
            move |(_, summary)| {
                seen.set(seen.get() + 1);
                assert!(!fail.get(), "sink failure");
                crate::SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 3))).unwrap();
    tx.apply(&source, TupleChange::put(1, (0, 4))).unwrap();
    tx.commit().unwrap();
    let original = graph.dataset_rows(&dataset).unwrap().clone();
    let count = seen.get();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, (0, 3))).unwrap();
        tx.apply(&source, TupleChange::put(0, (0, 5))).unwrap();
    }
    assert_eq!(seen.get(), count + 2);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &original);
    fail.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            let _ = tx.apply(&source, TupleChange::put(0, (2, -5)));
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &original);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 5))).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), crate::SimpleScore::new(9));
}

#[test]
fn sparse_pair_captures_preserve_snapshots_and_nested_trials_for_arbitrary_group_keys() {
    let mut node = GroupNode::<(usize, usize), String>::new();
    let changes = [
        ((0, 0), "first", 7),
        ((319, 319), "last", 9),
        ((0, 319), "first", -3),
    ];
    for ((a, b), key, value) in changes {
        node.apply(TupleChange::put((a, b), (key.into(), value)));
    }
    let original = node
        .groups()
        .map(|(k, v)| (k.clone(), v))
        .collect::<Vec<_>>();
    assert!(node.bind_pair_inputs(320, 320));
    assert!(!node.bind_pair_inputs(320, 320));
    let outer = node.begin_journal_checkpoint();
    node.apply(TupleChange::retract((319, 319)));
    node.apply(TupleChange::put((20, 319), ("middle".into(), i64::MAX)));
    let saved = node.checkpoint();
    let middle = node
        .groups()
        .map(|(k, v)| (k.clone(), v))
        .collect::<Vec<_>>();
    let inner = node.begin_journal_checkpoint();
    node.apply(TupleChange::put((0, 0), ("middle".into(), 1)));
    node.apply(TupleChange::retract((0, 319)));
    node.commit_journal_checkpoint(inner);
    node.restore_checkpoint(saved);
    assert_eq!(
        node.groups()
            .map(|(k, v)| (k.clone(), v))
            .collect::<Vec<_>>(),
        middle
    );
    node.clear();
    node.restore_checkpoint(outer);
    assert_eq!(
        node.groups()
            .map(|(k, v)| (k.clone(), v))
            .collect::<Vec<_>>(),
        original
    );
    node.apply(TupleChange::put((319, 319), ("last".into(), 10)));
    assert_eq!(
        node.groups()
            .find(|(key, _)| key.as_str() == "last")
            .unwrap()
            .1
            .sum,
        10
    );
}

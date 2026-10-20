//! Original primitive-storage regressions; no upstream-method credit.
use super::*;
use std::{cell::RefCell, rc::Rc};

fn contents<I: Ord, K: Ord + Clone>(node: &GroupNode<I, K>) -> Vec<(K, IntSummary)> {
    node.groups()
        .map(|(key, summary)| (key.clone(), summary))
        .collect()
}

#[test]
fn primitive_group_keys_match_tree_for_unbounded_coordinates_and_complete_multisets() {
    let mut tree = GroupNode::<usize, (usize, usize)>::new();
    let mut paged = GroupNode::new();
    assert!(paged.bind_primitive_group_keys());
    let coordinates = [0, 1, 15, 16, 31, 32, usize::MAX - 1, usize::MAX];
    let values = [0, 1, -1, i64::MAX, i64::MIN, 7, -7];
    let mut state = 19u64;
    for round in 0..1200 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (state >> 32) as usize % 29;
        let value = (!state.is_multiple_of(7)).then(|| {
            (
                (
                    coordinates[(state >> 40) as usize % coordinates.len()],
                    coordinates[(state >> 44) as usize % coordinates.len()],
                ),
                values[(state >> 48) as usize % values.len()],
            )
        });
        let change = TupleChange { id, value };
        if round % 17 == 0 {
            let changes = [change.clone(), TupleChange::retract((id + 1) % 29), change];
            assert_eq!(
                paged.apply_batch(changes.clone()),
                tree.apply_batch(changes),
                "round {round}"
            );
        } else {
            assert_eq!(
                paged.apply(change.clone()),
                tree.apply(change),
                "round {round}"
            );
        }
        assert_eq!(contents(&paged), contents(&tree), "round {round}");
    }
    for id in 0..29 {
        assert_eq!(
            paged.apply(TupleChange::retract(id)),
            tree.apply(TupleChange::retract(id))
        );
    }
    assert!(contents(&paged).is_empty());
}

#[test]
fn primitive_groups_restore_nested_commits_full_saves_clear_and_repeated_outer_trials() {
    let mut node = GroupNode::<usize, (usize, usize, usize, usize)>::new();
    assert!(node.bind_primitive_group_keys());
    for (id, key, value) in [
        (0, (usize::MAX, 0, 15, 16), 4),
        (1, (0, usize::MAX, 16, 15), -7),
        (2, (usize::MAX, 0, 15, 16), 3),
    ] {
        node.apply(TupleChange::put(id, (key, value)));
    }
    let original = contents(&node);
    for _ in 0..8 {
        let outer = node.begin_journal_checkpoint();
        node.apply(TupleChange::put(0, ((0, 0, 0, usize::MAX), i64::MAX)));
        node.apply(TupleChange::retract(1));
        let middle = contents(&node);
        let saved = node.checkpoint();
        let inner = node.begin_journal_checkpoint();
        node.apply(TupleChange::put(0, ((1, 1, 1, 1), 6)));
        node.apply(TupleChange::put(
            3,
            ((usize::MAX, usize::MAX, usize::MAX, usize::MAX), -6),
        ));
        node.commit_journal_checkpoint(inner);
        node.restore_checkpoint(saved);
        assert_eq!(contents(&node), middle);
        node.clear();
        node.restore_checkpoint(outer);
        assert_eq!(contents(&node), original);
    }
    node.apply(TupleChange::put(2, ((usize::MAX, 0, 15, 16), -4)));
    let remaining = contents(&node);
    assert_eq!(
        remaining
            .iter()
            .find(|(key, _)| *key == (usize::MAX, 0, 15, 16))
            .unwrap()
            .1
            .sum,
        0
    );
}

#[derive(Clone, Debug)]
struct Tagged(i32, i32);
impl PartialEq for Tagged {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Tagged {}
impl PartialOrd for Tagged {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Tagged {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[test]
fn custom_and_borrowed_keys_keep_tree_representatives_and_original_lifetime_bounds() {
    let mut node = GroupNode::<usize, Tagged>::new();
    assert!(!node.bind_primitive_group_keys());
    node.apply(TupleChange::put(0, (Tagged(1, 10), 2)));
    node.apply(TupleChange::put(0, (Tagged(1, 20), 3)));
    assert_eq!(node.groups().next().unwrap().0.1, 20);
    node.apply(TupleChange::put(1, (Tagged(1, 30), 4)));
    node.apply(TupleChange::put(0, (Tagged(1, 40), 5)));
    assert_eq!(node.groups().next().unwrap().0.1, 20);
    node.apply_batch([
        TupleChange::retract(0),
        TupleChange::put(1, (Tagged(1, 50), 7)),
    ]);
    assert_eq!(node.groups().next().unwrap().0.1, 20);
    let borrowed = String::from("local");
    let mut ordinary = GroupNode::new();
    ordinary.apply(TupleChange::put(0, (borrowed.as_str(), 1)));
    assert_eq!(ordinary.groups().next().unwrap().0, &"local");
}

#[test]
fn batched_primitive_summary_installation_recovers_after_partial_bound_input_failure() {
    let mut node = GroupNode::<usize, usize>::new();
    assert!(node.bind_primitive_group_keys());
    assert!(node.bind_fixed_inputs(2));
    node.apply(TupleChange::put(0, (usize::MAX, 5)));
    node.apply(TupleChange::put(1, (16, 7)));
    let original = contents(&node);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            node.apply_batch([TupleChange::put(0, (16, 9)), TupleChange::put(2, (0, -1))]);
        }))
        .is_err()
    );
    assert!(!node.is_poisoned());
    assert_eq!(contents(&node), original);
    node.apply_batch([TupleChange::put(0, (16, 9)), TupleChange::retract(1)]);
    assert_eq!(contents(&node)[0].1.sum, 9);
}

#[test]
fn graph_groups_emit_in_original_order_and_retain_datasets_through_commit_and_unwind() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let fail = Rc::new(std::cell::Cell::new(false));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let grouped = builder
        .group_collect(source.port(), |&(key, value)| ((key, usize::MAX), value))
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    builder
        .score("sum", &grouped, {
            let observed = observed.clone();
            let fail = fail.clone();
            move |(key, summary)| {
                observed.borrow_mut().push((*key, *summary));
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
    observed.borrow_mut().clear();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (64, 9))).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        observed
            .borrow()
            .iter()
            .map(|(key, summary)| (*key, summary.sum))
            .collect::<Vec<_>>(),
        [((0, usize::MAX), 4), ((64, usize::MAX), 9)]
    );
    assert_eq!(original[&(0, usize::MAX)].1.sum, 7);
    let committed = graph.dataset_rows(&dataset).unwrap().clone();
    fail.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            let _ = tx.apply(&source, TupleChange::put(0, (usize::MAX, -5)));
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &committed);
    assert_eq!(graph.score(), crate::SimpleScore::new(13));
    graph.assert_consistent().unwrap();
}

fn exercise_same_base_capture<
    I: Ord + Clone + 'static,
    K: Ord + Clone + std::fmt::Debug + 'static,
>(
    id: impl Fn(usize) -> I,
    key: impl Fn(usize, usize) -> K,
    binding: usize,
) {
    let mut node = GroupNode::<I, K>::new();
    node.bind_primitive_group_keys();
    for (i, k, v) in [(0, 0, 3), (1, 1, 4), (2, 0, 5), (5, 2, 7)] {
        node.apply(TupleChange::put(id(i), (key(k, 10 + i), v)));
    }
    match binding {
        1 => assert!(node.bind_fixed_inputs(1024)),
        2 => assert!(node.bind_pair_inputs(573, 573)),
        3 => assert!(node.bind_primitive_inputs()),
        _ => {}
    }
    let view = |node: &GroupNode<I, K>| format!("{:?}", contents(node));
    let base = view(&node);
    let full_base = node.checkpoint();
    let outer = node.begin_journal_checkpoint();
    node.apply(TupleChange::put(id(0), (key(4, 40), i64::MAX)));
    let inner = node.begin_journal_checkpoint();
    node.apply(TupleChange::retract(id(1)));
    node.apply(TupleChange::put(id(2), (key(0, 42), -5)));
    node.apply(TupleChange::put(id(3), (key(6, 43), 2)));
    node.apply(TupleChange::put(id(3), (key(6, 44), -2)));
    let expected = view(&node);
    let captured = node.capture_checkpoint(outer.as_ref());
    // Capturing is read-only and includes the still-open nested frame.
    assert_eq!(view(&node), expected);
    node.restore_checkpoint(inner);
    node.restore_checkpoint(outer);
    assert_eq!(view(&node), base);
    // Unselected trials over the unchanged base cannot invalidate the capture.
    let other = node.begin_journal_checkpoint();
    node.apply(TupleChange::put(id(0), (key(8, 80), -9)));
    node.apply(TupleChange::put(id(4), (key(9, 90), 11)));
    node.restore_checkpoint(other);
    assert_eq!(view(&node), base);
    let install_scope = node.begin_journal_checkpoint();
    node.restore_checkpoint(captured);
    assert_eq!(view(&node), expected);
    let captured_again = node.capture_checkpoint(install_scope.as_ref());
    node.clear();
    node.restore_checkpoint(install_scope);
    assert_eq!(
        view(&node),
        base,
        "install must preserve an enclosing journal"
    );
    node.restore_checkpoint(captured_again);
    assert_eq!(view(&node), expected);
    for i in 0..6 {
        node.apply(TupleChange::retract(id(i)));
    }
    assert!(
        node.groups().next().is_none(),
        "captured inputs must match captured summaries"
    );
    node.restore_checkpoint(full_base);
    assert_eq!(view(&node), base);
}

#[test]
fn capture_keeps_only_current_bound_changes_and_preserves_generic_full_fallback() {
    exercise_same_base_capture(|i| i, |k, _| k, 1);
    exercise_same_base_capture(|i| (i * 80, 572 - i), |k, _| (k, usize::MAX), 2);
    exercise_same_base_capture(
        |i| format!("id-{i}"),
        |k, tag| Tagged(k as i32, tag as i32),
        0,
    );
}

#[test]
fn bound_group_capture_retains_untouched_custom_group_key_owners() {
    let mut node = GroupNode::<usize, Rc<usize>>::new();
    assert!(
        !node.bind_primitive_inputs(),
        "custom Drop keys keep ordinary input ownership"
    );
    let key = Rc::new(7usize);
    let weak = Rc::downgrade(&key);
    node.apply(TupleChange::put(0, (key, 1)));
    assert!(node.bind_fixed_inputs(1));
    let original = node.begin_journal_checkpoint();
    let saved = node.capture_checkpoint(original.as_ref());
    node.restore_checkpoint(original);
    node.clear();
    assert!(weak.upgrade().is_some());
    drop(saved);
    assert!(weak.upgrade().is_none());
}

#[test]
fn graph_captured_group_winner_installs_without_replay_after_other_trials() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let grouped = builder
        .group_collect(source.port(), |&(key, value)| ((key, usize::MAX), value))
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    builder
        .score("sum", &grouped, {
            let calls = calls.clone();
            move |(_, summary)| {
                calls.set(calls.get() + 1);
                crate::SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 3))).unwrap();
    tx.apply(&source, TupleChange::put(1, (0, 4))).unwrap();
    tx.commit().unwrap();
    let base = graph.dataset_rows(&dataset).unwrap().clone();
    let (saved, expected) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, (64, 9))).unwrap();
        let expected = tx.graph().unwrap().dataset_rows(&dataset).unwrap().clone();
        (tx.capture().unwrap(), expected)
    };
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &base);
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(1, (usize::MAX, -5)))
            .unwrap();
    }
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &base);
    let before = calls.get();
    graph.install(saved);
    assert_eq!(
        calls.get(),
        before,
        "install may not replay mapping/scoring callbacks"
    );
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &expected);
    assert_eq!(graph.score(), crate::SimpleScore::new(13));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), crate::SimpleScore::new(4));
    graph.assert_consistent().unwrap();
}

#[test]
fn unbounded_input_capture_preserves_selected_expansion_occurrences_and_full_fallbacks() {
    exercise_same_base_capture(
        |i| (usize::MAX - i, usize::MAX - i * 64),
        |k, _| (k, usize::MAX, 0, usize::MAX),
        3,
    );
    let local = String::from("borrowed ID");
    let mut borrowed = GroupNode::<&str, usize>::new();
    borrowed.apply(TupleChange::put(local.as_str(), (0, 3)));
    assert_eq!(borrowed.groups().next().unwrap().1.sum, 3);
    let mut generic = GroupNode::<String, usize>::new();
    assert!(!generic.bind_primitive_inputs());
    generic.apply(TupleChange::put(String::from("custom ID"), (usize::MAX, 7)));
    let saved = generic.begin_journal_checkpoint();
    generic.apply(TupleChange::put(String::from("custom ID"), (0, 8)));
    let captured = generic.capture_checkpoint(saved.as_ref());
    generic.restore_checkpoint(saved);
    generic.restore_checkpoint(captured);
    assert_eq!(generic.groups().next().unwrap().1.sum, 8);
}

#[test]
fn graph_flat_map_group_input_capture_matches_fresh_full_rebuild_without_replay() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, Vec<(usize, i64)>>();
    let expanded = builder
        .flat_map(source.port(), |rows| rows.clone())
        .unwrap();
    let grouped = builder
        .group_collect(&expanded, |&(key, value)| {
            ((key, usize::MAX, 0, usize::MAX), value)
        })
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    builder
        .score("sum", &grouped, {
            let calls = calls.clone();
            move |(_, summary)| {
                calls.set(calls.get() + 1);
                crate::SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, vec![(0, 1), (0, 2), (16, 3)]))
        .unwrap();
    tx.apply(&source, TupleChange::put(usize::MAX, vec![(usize::MAX, 4)]))
        .unwrap();
    tx.commit().unwrap();
    let base = graph.dataset_rows(&dataset).unwrap().clone();
    let (captured, expected) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, vec![(16, 7)]))
            .unwrap();
        tx.apply(
            &source,
            TupleChange::put(usize::MAX, vec![(0, -1), (usize::MAX, 9)]),
        )
        .unwrap();
        let expected = tx.graph().unwrap().dataset_rows(&dataset).unwrap().clone();
        (tx.capture().unwrap(), expected)
    };
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &base);
    {
        let mut tx = graph.transaction();
        tx.apply(
            &source,
            TupleChange::put(0, vec![(31, 11), (32, 12), (33, 13), (34, 14)]),
        )
        .unwrap();
    }
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &base);
    let before = calls.get();
    graph.install(captured);
    assert_eq!(calls.get(), before);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &expected);
    assert_eq!(graph.score(), crate::SimpleScore::new(15));
    graph.assert_consistent().unwrap();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.apply(&source, TupleChange::retract(usize::MAX)).unwrap();
    tx.commit().unwrap();
    assert!(graph.dataset_rows(&dataset).unwrap().is_empty());
    assert_eq!(graph.score(), crate::SimpleScore::ZERO);
}

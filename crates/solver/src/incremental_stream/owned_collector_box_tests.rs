//! Original independent multiset oracle for consumed collector ownership.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
type Key = (usize, usize, usize, usize);
type Inputs = BTreeMap<usize, (Key, i64)>;
fn expected(rows: &Inputs) -> Vec<(Key, IntSummary)> {
    let mut groups = BTreeMap::<Key, Vec<i64>>::new();
    for &(key, value) in rows.values() {
        groups.entry(key).or_default().push(value);
    }
    groups
        .into_iter()
        .map(|(key, values)| {
            let distinct = values.iter().copied().collect::<BTreeSet<_>>();
            (
                key,
                IntSummary {
                    count: values.len(),
                    distinct_count: distinct.len(),
                    sum: values.iter().fold(0i64, |a, b| a.wrapping_add(*b)),
                    min: values.iter().copied().min(),
                    max: values.iter().copied().max(),
                },
            )
        })
        .collect()
}
fn check(node: &GroupNode<usize, Key>, rows: &Inputs) {
    assert_eq!(
        node.groups().map(|(key, s)| (*key, s)).collect::<Vec<_>>(),
        expected(rows)
    );
}
fn edit(
    node: &mut GroupNode<usize, Key>,
    shared: &mut GroupNode<usize, Key>,
    rows: &mut Inputs,
    seed: &mut u64,
) {
    let ids = [0, 1, 15, 16, usize::MAX];
    let keys = [
        (0, 0, 0, 0),
        (0, 0, 0, 15),
        (0, 0, 0, 16),
        (1, 2, 3, 4),
        (127, 15, 127, 63),
        (128, 16, 1024, 64),
        (usize::MAX, usize::MAX, usize::MAX, usize::MAX),
    ];
    let values = [i64::MIN, i64::MAX, -7, 0, 7, 7, 1];
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let id = ids[(*seed >> 32) as usize % ids.len()];
    let next = (*seed & 15 != 0).then(|| {
        (
            keys[(*seed >> 16) as usize % keys.len()],
            values[(*seed >> 8) as usize % values.len()],
        )
    });
    let change = TupleChange { id, value: next };
    assert_eq!(node.apply(change.clone()), shared.apply(change));
    if let Some(value) = next {
        rows.insert(id, value);
    } else {
        rows.remove(&id);
    }
    check(node, rows);
    check(shared, rows);
}
#[test]
fn owned_collector_box_history_matches_independent_groups_and_shared_events() {
    let mut node = GroupNode::<usize, Key>::new();
    let mut shared = GroupNode::<usize, Key>::new();
    assert!(node.bind_primitive_group_keys());
    assert!(shared.bind_primitive_group_keys());
    assert!(node.bind_primitive_inputs());
    assert!(shared.bind_primitive_inputs());
    node.enable_consumed_collectors();
    let mut rows = Inputs::new();
    let mut seed = 0xbadc0ffee_u64;
    for _ in 0..16 {
        edit(&mut node, &mut shared, &mut rows, &mut seed);
    }
    for round in 0..24 {
        let baseline = rows.clone();
        let outer = node.begin_journal_checkpoint();
        let s_outer = shared.begin_journal_checkpoint();
        for _ in 0..12 {
            edit(&mut node, &mut shared, &mut rows, &mut seed);
        }
        let captured_rows = rows.clone();
        let capture = node.capture_checkpoint(outer.as_ref());
        let s_capture = shared.capture_checkpoint(s_outer.as_ref());
        let full = node.checkpoint();
        let s_full = shared.checkpoint();
        let child = node.begin_journal_checkpoint();
        let s_child = shared.begin_journal_checkpoint();
        for _ in 0..8 {
            edit(&mut node, &mut shared, &mut rows, &mut seed);
        }
        if round % 2 == 0 {
            node.commit_journal_checkpoint(child);
            shared.commit_journal_checkpoint(s_child);
        } else {
            node.restore_checkpoint(child);
            shared.restore_checkpoint(s_child);
            rows = captured_rows.clone();
        }
        check(&node, &rows);
        check(&shared, &rows);
        if round % 3 == 0 {
            node.clear();
            shared.clear();
            rows.clear();
            check(&node, &rows);
            for _ in 0..3 {
                edit(&mut node, &mut shared, &mut rows, &mut seed);
            }
        }
        node.restore_checkpoint(full);
        shared.restore_checkpoint(s_full);
        rows = captured_rows.clone();
        check(&node, &rows);
        check(&shared, &rows);
        node.restore_checkpoint(outer);
        shared.restore_checkpoint(s_outer);
        rows = baseline;
        check(&node, &rows);
        check(&shared, &rows);
        let install = node.begin_journal_checkpoint();
        let s_install = shared.begin_journal_checkpoint();
        node.restore_checkpoint(capture);
        shared.restore_checkpoint(s_capture);
        check(&node, &captured_rows);
        check(&shared, &captured_rows);
        node.restore_checkpoint(install);
        shared.restore_checkpoint(s_install);
        check(&node, &rows);
        check(&shared, &rows);
        for _ in 0..3 {
            edit(&mut node, &mut shared, &mut rows, &mut seed);
        }
    }
}

#[test]
fn owned_collector_arena_growth_retirement_and_snapshot_installs_match_independent_groups() {
    let mut node = GroupNode::<usize, Key>::new();
    assert!(node.bind_primitive_group_keys());
    assert!(node.bind_primitive_inputs());
    node.enable_consumed_collectors();
    let key = |i: usize| {
        if i % 19 == 0 {
            (usize::MAX, i, usize::MAX - i, 15)
        } else {
            (i / 16, i % 16, i * 3, i * 7)
        }
    };
    let value = |i: usize| [i64::MIN, i64::MAX, 0, -7, 7][i % 5];
    let mut rows = Inputs::new();
    for i in 0..257 {
        let row = (key(i), value(i));
        node.apply(TupleChange::put(i, row));
        rows.insert(i, row);
    }
    check(&node, &rows);
    let baseline = rows.clone();
    let baseline_full = node.checkpoint();
    let outer = node.begin_journal_checkpoint();
    for i in (0..257).step_by(2) {
        node.apply(TupleChange::retract(i));
        rows.remove(&i);
    }
    for i in 257..1027 {
        let row = (key(i), value(i));
        node.apply(TupleChange::put(i, row));
        rows.insert(i, row);
    }
    check(&node, &rows);
    let captured_rows = rows.clone();
    let full = node.checkpoint();
    let full_again = node.checkpoint();
    let delta = node.capture_checkpoint(outer.as_ref());
    let inner = node.begin_journal_checkpoint();
    node.clear();
    for i in 1100..1500 {
        node.apply(TupleChange::put(i, (key(i), value(i))));
    }
    node.restore_checkpoint(inner);
    check(&node, &captured_rows);
    node.restore_checkpoint(full);
    check(&node, &captured_rows);
    node.restore_checkpoint(outer);
    check(&node, &baseline);
    let install = node.begin_journal_checkpoint();
    node.restore_checkpoint(delta);
    check(&node, &captured_rows);
    node.restore_checkpoint(install);
    check(&node, &baseline);
    // No open frame: full state replaces the arena and its local slot namespace.
    node.restore_checkpoint(full_again);
    check(&node, &captured_rows);
    for i in 257..1027 {
        node.apply(TupleChange::retract(i));
    }
    for i in 1600..2200 {
        node.apply(TupleChange::put(i, (key(i), value(i))));
    }
    node.clear();
    assert_eq!(node.groups().count(), 0);
    node.restore_checkpoint(baseline_full);
    check(&node, &baseline);
    // Reuse retired slots after all journal references have gone away.
    for i in 0..257 {
        node.apply(TupleChange::retract(i));
    }
    rows.clear();
    for i in 2500..3000 {
        let row = (key(i), value(i));
        node.apply(TupleChange::put(i, row));
        rows.insert(i, row);
    }
    check(&node, &rows);
}

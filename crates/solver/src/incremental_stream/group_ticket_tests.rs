//! Original immutable summary-output controls; no upstream-method credit.
use super::*;
use crate::primitive_rows::Exact;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
type Pair = (usize, usize);
type Node = GroupNode<Pair, Pair>;
fn contents(n: &Node) -> Vec<(Pair, IntSummary)> {
    n.groups().map(|(k, v)| (*k, v)).collect()
}
fn output(
    n: &mut Node,
    change: TupleChange<Pair, (Pair, i64)>,
) -> [Option<TupleChange<Pair, IntSummary>>; 2] {
    n.apply_summary_seeds::<Exact, _>(change, |old, new| {
        [
            old.map(|row| row.map(|collector| collector.materialize())),
            new.map(|row| row.map(|collector| collector.materialize())),
        ]
    })
}
fn setup(mode: usize) -> Node {
    let mut n = Node::new();
    assert!(n.bind_primitive_group_keys());
    if mode != 0 {
        assert!(n.bind_primitive_inputs());
    }
    if mode == 3 {
        n.enable_owned_inputs();
    }
    if mode >= 2 {
        assert!(n.bind_pair_inputs(4, 4));
    }
    n.enable_consumed_collectors();
    assert!(n.has_owned_summary_groups());
    n
}
fn step(a: &mut Node, b: &mut Node, c: TupleChange<Pair, (Pair, i64)>) {
    assert_eq!(
        output(a, c.clone()),
        b.apply_inline_with_policy::<Exact, _>(c, |old, new| [old, new])
    );
    assert_eq!(contents(a), contents(b));
    assert!(!a.is_poisoned());
}
#[test]
fn summary_seeds_preserve_full_rows_inputs_growth_and_nested_snapshot_namespaces() {
    let keys = [(0, 15), (0, 16), (1, usize::MAX), (usize::MAX, usize::MAX)];
    for mode in 0..4 {
        let (mut a, mut b) = (setup(mode), setup(mode));
        for (id, key, value) in [
            ((0, 0), keys[0], i64::MIN),
            ((0, 1), keys[0], i64::MAX),
            ((1, 0), keys[1], 7),
            ((1, 1), keys[2], -7),
            ((2, 0), keys[3], 0),
        ] {
            step(&mut a, &mut b, TupleChange::put(id, (key, value)));
        }
        if mode < 2 {
            step(
                &mut a,
                &mut b,
                TupleChange::put((usize::MAX, usize::MAX), (keys[3], 11)),
            );
        }
        let base = contents(&a);
        let full = a.checkpoint();
        let old_full = b.checkpoint();
        let outer = a.begin_journal_checkpoint();
        let old_outer = b.begin_journal_checkpoint();
        for c in [
            TupleChange::put((0, 0), (keys[1], 9)),
            TupleChange::put((1, 0), (keys[2], i64::MIN)),
            TupleChange::put((1, 1), (keys[2], -7)),
            TupleChange::retract((0, 1)),
            TupleChange::retract((3, 3)),
            TupleChange::put((0, 1), (keys[3], i64::MAX)),
        ] {
            step(&mut a, &mut b, c);
        }
        let inner = a.begin_journal_checkpoint();
        let old_inner = b.begin_journal_checkpoint();
        step(&mut a, &mut b, TupleChange::put((0, 0), (keys[3], 42)));
        step(&mut a, &mut b, TupleChange::retract((1, 0)));
        let wanted = contents(&a);
        let saved = a.capture_checkpoint(outer.as_ref());
        let old_saved = b.capture_checkpoint(old_outer.as_ref());
        a.commit_journal_checkpoint(inner);
        b.commit_journal_checkpoint(old_inner);
        a.restore_checkpoint(outer);
        b.restore_checkpoint(old_outer);
        assert_eq!(contents(&a), base);
        assert_eq!(contents(&a), contents(&b));
        let t = a.begin_journal_checkpoint();
        let old_t = b.begin_journal_checkpoint();
        a.restore_checkpoint(saved);
        b.restore_checkpoint(old_saved);
        assert_eq!(contents(&a), wanted);
        assert_eq!(contents(&a), contents(&b));
        a.clear();
        b.clear();
        assert!(contents(&a).is_empty());
        a.restore_checkpoint(full);
        b.restore_checkpoint(old_full);
        assert_eq!(contents(&a), base);
        a.restore_checkpoint(t);
        b.restore_checkpoint(old_t);
        assert_eq!(contents(&a), base);
        assert_eq!(contents(&a), contents(&b));
    }
    // Each transfer retains an old nonempty group while creating a new group.
    // Many sizes exercise slot-vector and directory growth during the edit.
    for size in [3usize, 7, 15, 63, 127, 511] {
        let (mut a, mut b) = (setup(1), setup(1));
        for n in 0..size {
            step(&mut a, &mut b, TupleChange::put((n, 0), ((n, 0), n as i64)));
        }
        step(&mut a, &mut b, TupleChange::put((size, 0), ((0, 0), -3)));
        step(
            &mut a,
            &mut b,
            TupleChange::put((0, 0), ((usize::MAX, size), i64::MAX)),
        );
        let got: BTreeMap<_, _> = contents(&a).into_iter().collect();
        assert_eq!(got[&(0, 0)].count, 1);
        assert_eq!(got[&(0, 0)].sum, -3);
        assert_eq!(got[&(usize::MAX, size)].count, 1);
        assert_eq!(got[&(usize::MAX, size)].max, Some(i64::MAX));
    }
}
#[test]
fn summary_seeds_keep_late_bounds_poison_and_never_publish_before_recovery() {
    for owned_input in [false, true] {
        let mut a = GroupNode::<usize, Pair>::new();
        let mut b = GroupNode::<usize, Pair>::new();
        for n in [&mut a, &mut b] {
            assert!(n.bind_primitive_group_keys());
            if owned_input {
                n.enable_owned_inputs();
            }
            assert!(n.bind_fixed_inputs(2));
            n.enable_consumed_collectors();
            n.apply(TupleChange::put(0, ((0, 0), 7)));
        }
        let base: Vec<_> = a.groups().map(|(k, v)| (*k, v)).collect();
        let saved = a.checkpoint();
        let old_saved = b.checkpoint();
        let mut called = false;
        let fail = catch_unwind(AssertUnwindSafe(|| {
            a.apply_summary_seeds::<Exact, _>(TupleChange::put(2, ((1, 1), 9)), |_, _| {
                called = true;
            })
        }));
        let old_fail = catch_unwind(AssertUnwindSafe(|| {
            b.apply_inline(TupleChange::put(2, ((1, 1), 9)))
        }));
        assert!(fail.is_err() && old_fail.is_err());
        assert!(!called);
        assert!(a.is_poisoned() && b.is_poisoned());
        a.restore_checkpoint(saved);
        b.restore_checkpoint(old_saved);
        assert_eq!(a.groups().map(|(k, v)| (*k, v)).collect::<Vec<_>>(), base);
        let emitted =
            a.apply_summary_seeds::<Exact, _>(TupleChange::put(1, ((0, 0), -7)), |old, new| {
                [
                    old.map(|r| r.map(|c| c.materialize())),
                    new.map(|r| r.map(|c| c.materialize())),
                ]
            });
        assert_eq!(emitted, b.apply_inline(TupleChange::put(1, ((0, 0), -7))));
    }
}

#[test]
fn summary_seed_values_remain_immutable_after_later_group_edits() {
    let mut node = setup(1);
    output(&mut node, TupleChange::put((0, 0), ((0, 0), 7)));
    let captured = node
        .apply_summary_seeds::<Exact, _>(TupleChange::put((1, 0), ((0, 0), 9)), |old, new| {
            [old, new]
        });
    output(&mut node, TupleChange::put((0, 0), ((0, 0), i64::MIN)));
    node.clear();
    let materialized = captured.map(|r| r.map(|r| r.map(|s| s.materialize())));
    assert_eq!(
        materialized[1].as_ref().unwrap().value,
        Some(IntSummary {
            count: 2,
            distinct_count: 2,
            sum: 16,
            min: Some(7),
            max: Some(9)
        })
    );
}

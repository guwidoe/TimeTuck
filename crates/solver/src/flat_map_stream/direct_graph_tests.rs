//! Original direct-erasure controls, no upstream adaptation credit.
use super::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
#[test]
fn direct_graph_output_preserves_full_rows_zero_presence_tails_equal_owners_and_captures() {
    type Id = (usize, usize);
    type Row = (usize, usize, usize, usize, i64);
    type Run = (
        Vec<Vec<TupleChange<(Id, usize), Row>>>,
        Vec<(Id, usize)>,
        usize,
    );
    fn run(direct: bool) -> Run {
        let calls = Cell::new(0usize);
        let mut node = FlatMapNode::new(|&(n, value): &(usize, i64)| {
            calls.set(calls.get() + 1);
            (0..n)
                .map(|p| (usize::MAX, n, p, usize::MAX - 1, value))
                .collect::<Vec<_>>()
        });
        assert!(node.bind_graph_inputs());
        fn apply(
            node: &mut FlatMapNode<'_, Id, (usize, i64), Row>,
            direct: bool,
            c: TupleChange<Id, (usize, i64)>,
        ) -> Vec<TupleChange<(Id, usize), Row>> {
            if direct {
                node.apply_graph_output(c, |id, tail, rows| {
                    let rows = tail.map(|p| TupleChange::retract((*id, p))).chain(
                        rows.enumerate()
                            .map(|(p, row)| TupleChange::put((*id, p), row)),
                    );
                    let (n, upper) = rows.size_hint();
                    assert_eq!(upper, Some(n));
                    let out: Vec<_> = rows.collect();
                    assert_eq!(n, out.len());
                    out
                })
            } else {
                node.apply(c)
            }
        }
        let id = (usize::MAX, usize::MAX - 1);
        let other = (usize::MAX - 1, usize::MAX);
        let mut outputs = Vec::new();
        for (n, value) in [
            (0, i64::MIN),
            (0, i64::MAX),
            (3, i64::MIN),
            (3, i64::MAX),
            (1, 0),
            (0, 1),
            (0, 2),
        ] {
            let held = node.lengths.shared_entries();
            let old = node.lengths.get(&id).copied();
            outputs.push(apply(&mut node, direct, TupleChange::put(id, (n, value))));
            assert_eq!(node.lengths.get(&id), Some(&n));
            if old == Some(n) {
                let now = node.lengths.shared_entries();
                assert!(std::sync::Arc::ptr_eq(&held[0].1, &now[0].1));
            }
        }
        let full = node.checkpoint();
        let outer = node.begin_checkpoint();
        outputs.push(apply(
            &mut node,
            direct,
            TupleChange::put(id, (4, i64::MAX)),
        ));
        let inner = node.begin_checkpoint();
        outputs.push(apply(
            &mut node,
            direct,
            TupleChange::put(other, (2, i64::MIN)),
        ));
        let saved = node.capture_checkpoint(outer.as_ref());
        node.commit_checkpoint(inner);
        node.restore(outer);
        assert_eq!(node.lengths.get(&id), Some(&0));
        assert!(node.lengths.get(&other).is_none());
        let install = node.begin_checkpoint();
        node.restore(saved);
        assert_eq!(node.lengths.get(&id), Some(&4));
        assert_eq!(node.lengths.get(&other), Some(&2));
        node.restore(install);
        outputs.push(apply(&mut node, direct, TupleChange::retract(id)));
        assert!(node.lengths.get(&id).is_none());
        node.restore(full);
        let state = node.lengths.iter().map(|(id, n)| (*id, *n)).collect();
        (outputs, state, calls.get())
    }
    assert_eq!(run(false), run(true));
}
#[test]
fn direct_graph_output_erasure_panic_and_capacity_preflight_do_not_publish_lengths() {
    let mut node = FlatMapNode::new(|n: &usize| 0..*n);
    assert!(node.bind_graph_inputs());
    node.apply(TupleChange::put(usize::MAX, 2usize));
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply_graph_output(
            TupleChange::put(usize::MAX, 5usize),
            |id, tail, rows| {
                let mut rows = tail.map(|p| TupleChange::retract((*id, p))).chain(
                    rows.enumerate()
                        .map(|(p, row)| TupleChange::put((*id, p), row)),
                );
                assert_eq!(rows.next().unwrap(), TupleChange::put((usize::MAX, 0), 0));
                panic!("output erasure failure")
            }
        )))
        .is_err()
    );
    assert_eq!(node.lengths.get(&usize::MAX), Some(&2));
    let out: Vec<_> =
        node.apply_graph_output(TupleChange::put(usize::MAX, 0usize), |id, tail, rows| {
            tail.map(|p| TupleChange::retract((*id, p)))
                .chain(
                    rows.enumerate()
                        .map(|(p, row)| TupleChange::put((*id, p), row)),
                )
                .collect()
        });
    assert_eq!(
        out,
        vec![
            TupleChange::retract((usize::MAX, 0)),
            TupleChange::retract((usize::MAX, 1))
        ]
    );
    assert_eq!(node.lengths.get(&usize::MAX), Some(&0));
    // Vec<()> represents this finite cardinality without allocating or iterating
    // it. Collecting actual nonzero-sized output tuples must reject capacity
    // before any item/cache mutation, rather than running a gigantic loop.
    let mut huge = FlatMapNode::new(|n: &usize| vec![(); *n]);
    assert!(huge.bind_graph_inputs());
    huge.apply(TupleChange::put(0usize, 0usize));
    assert!(
        catch_unwind(AssertUnwindSafe(|| huge.apply_graph_output(
            TupleChange::put(0usize, usize::MAX),
            |id, tail, rows| {
                let rows = tail.map(|p| TupleChange::retract((*id, p))).chain(
                    rows.enumerate()
                        .map(|(p, row)| TupleChange::put((*id, p), row)),
                );
                rows.collect::<Vec<_>>()
            }
        )))
        .is_err()
    );
    assert_eq!(huge.lengths.get(&0), Some(&0));
}

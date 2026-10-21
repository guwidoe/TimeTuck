//! Original graph-only input ownership/callback controls; no upstream credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
type Row = (usize, usize, usize);
type Key = (usize, usize);
type Trace = Vec<(&'static str, usize, usize)>;
type Observation = (
    Trace,
    SimpleScore,
    Vec<(usize, Row)>,
    Vec<(Key, IntSummary)>,
);
fn collect<'a, I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static>(
    b: &mut StreamGraphBuilder<'a, SimpleScore>,
    input: &StreamPort<I, T>,
    legacy: bool,
    mapping: impl Fn(&T) -> (K, i64) + 'a,
) -> StreamPort<K, (K, IntSummary)> {
    if !legacy {
        return b.group_collect(input, mapping).unwrap();
    }
    let mut node = GroupNode::<I, K>::new();
    node.bind_primitive_group_keys();
    node.bind_primitive_inputs();
    node.enable_equal_input_reuse();
    let out = b.push(Group {
        node,
        mapping: Box::new(mapping),
        marker: PhantomData,
    });
    b.connect(input.node, out.node, false);
    out
}
fn run(legacy: bool) -> Observation {
    let trace = RefCell::new(Vec::new());
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let present = b
        .filter(source.port(), |r| {
            trace.borrow_mut().push(("present", r.0, r.2));
            true
        })
        .unwrap();
    let assigned = b.map(&present, |r| *r).unwrap();
    let loads = collect(&mut b, &assigned, legacy, |r| {
        trace.borrow_mut().push(("load map", r.0, r.2));
        ((r.1, r.2), 1)
    });
    let load_data = b.dataset(&loads).unwrap();
    let constraint = b
        .score("loads", &loads, |(_, s)| {
            trace.borrow_mut().push(("load score", s.count, 0));
            assert!(!fail.get(), "score panic");
            SimpleScore::new(-s.sum * s.sum)
        })
        .unwrap();
    let joined = b
        .join_equal(&assigned, &assigned, |r| (r.1, r.2), |r| (r.1, r.2))
        .unwrap();
    let pairs = b
        .filter(&joined, |(a, b)| {
            trace.borrow_mut().push(("pair filter", a.0, b.0));
            a.0 < b.0
        })
        .unwrap();
    let contacts = collect(&mut b, &pairs, legacy, |(a, b)| {
        trace.borrow_mut().push(("contact map", a.0, b.0));
        ((a.0, b.0), 1)
    });
    b.score("contacts", &contacts, |(_, s)| {
        trace.borrow_mut().push(("contact score", s.count, 0));
        SimpleScore::new(s.sum)
    })
    .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        for id in 0usize..3 {
            tx.apply(&source, TupleChange::put(id, (id, 0, usize::MAX)))
                .unwrap();
        }
        tx.commit().unwrap();
    }
    g.bind_fixed_planning_source(&source, 3).unwrap();
    trace.borrow_mut().clear();
    let base = g.capture_full();
    let base_rows = g.source_rows(&source).unwrap().clone();
    let public_before = g.constraint_matches(&constraint).unwrap().clone();
    let before = public_before.shared_entries();
    let (duplicate, changed, public_after, evaluations) = {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0, 0, usize::MAX)))
            .unwrap();
        let public_after = tx.graph.constraint_matches(&constraint).unwrap().clone();
        let after = public_after.shared_entries();
        assert!(
            !Arc::ptr_eq(&before[0].1, &after[0].1),
            "equal group input still publishes fresh public match payloads"
        );
        let duplicate = tx.capture().unwrap();
        tx.apply(&source, TupleChange::put(0usize, (0, 0, 1)))
            .unwrap();
        let evaluations = tx.score_evaluations();
        (duplicate, tx.capture().unwrap(), public_after, evaluations)
    };
    trace
        .borrow_mut()
        .push(("evaluations", evaluations as usize, 0));
    assert_eq!(g.source_rows(&source).unwrap(), &base_rows);
    assert!(Arc::ptr_eq(
        &before[0].1,
        &g.constraint_matches(&constraint).unwrap().shared_entries()[0].1
    ));
    let after = public_after.shared_entries();
    assert!(!Arc::ptr_eq(&before[0].1, &after[0].1));
    let calls = trace.borrow().len();
    g.install(duplicate);
    assert_eq!(trace.borrow().len(), calls);
    assert!(Arc::ptr_eq(
        &after[0].1,
        &g.constraint_matches(&constraint).unwrap().shared_entries()[0].1
    ));
    g.install(base);
    {
        let mut outer = g.transaction();
        for i in 0..outer.graph.nodes.len() {
            outer.checkpoint(i);
        }
        outer.graph.install(changed);
        assert_eq!(outer.graph.source_rows(&source).unwrap()[&0], (0, 0, 1));
    }
    assert_eq!(g.source_rows(&source).unwrap(), &base_rows);
    assert!(Arc::ptr_eq(
        &before[0].1,
        &g.constraint_matches(&constraint).unwrap().shared_entries()[0].1
    ));
    fail.set(true);
    {
        let mut tx = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0usize, (0, 0, 1)))
            ))
            .is_err()
        );
        assert!(tx.score().is_err());
    }
    fail.set(false);
    assert_eq!(g.source_rows(&source).unwrap(), &base_rows);
    {
        let mut tx = g.transaction();
        assert!(matches!(
            tx.apply(&source, TupleChange::put(usize::MAX, (0, 0, 1))),
            Err(GraphError::Dependency(_))
        ));
    }
    assert_eq!(g.source_rows(&source).unwrap(), &base_rows);
    g.assert_consistent().unwrap();
    let score = g.score();
    let source_rows = g
        .source_rows(&source)
        .unwrap()
        .iter()
        .map(|(i, r)| (*i, *r))
        .collect();
    let loads = g
        .dataset_rows(&load_data)
        .unwrap()
        .iter()
        .map(|(k, (_, s))| (*k, *s))
        .collect();
    drop(g);
    (trace.into_inner(), score, source_rows, loads)
}
#[test]
fn owned_group_inputs_graph_bound_loads_contacts_captures_public_owners_and_panics_match_original()
{
    assert_eq!(run(false), run(true));
}

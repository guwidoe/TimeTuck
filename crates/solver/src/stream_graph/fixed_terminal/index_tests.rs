//! Original ordinal-index controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{cell::RefCell, rc::Rc};

fn terminal<I: Ord + Clone + 'static>(
    mut ids: Vec<I>,
) -> FixedTerminal<'static, I, i64, SimpleScore> {
    ids.sort();
    FixedTerminal {
        direct_ordinals: contiguous_ordinals(&ids),
        positions: ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(slot, id)| (id, slot))
            .collect(),
        rows: Slots::new(ids.len()),
        ids,
        materialized: OnceCell::new(),
        evaluate: TerminalEvaluation::Direct(Box::new(|v| SimpleScore::new(*v))),
        total: SimpleScore::ZERO,
        checkpoint_box: CheckpointBox::new(),
        name: "index".to_owned(),
    }
}
#[test]
fn primitive_contiguous_sets_enable_direct_lookup_and_sparse_empty_extremes_fall_back() {
    for count in [1, 256, 257] {
        let node = terminal((0..count).rev().collect::<Vec<usize>>());
        assert!(node.direct_ordinals);
        assert_eq!(node.position(&0), Some(0));
        assert_eq!(node.position(&(count - 1)), Some(count - 1));
        assert_eq!(node.position(&count), None);
        assert_eq!(node.position(&usize::MAX), None);
    }
    for ids in [vec![], vec![usize::MAX], vec![0, usize::MAX], vec![0, 2]] {
        let node = terminal(ids);
        assert!(!node.direct_ordinals);
        for query in [0, 1, 2, usize::MAX] {
            assert_eq!(node.position(&query), node.positions.get(&query).copied());
        }
    }
    let text = terminal(vec!["a".to_owned(), "z".to_owned()]);
    assert!(!text.direct_ordinals);
    assert_eq!(text.position(&"z".to_owned()), Some(1));
}
#[test]
fn relationship_ordinals_validate_full_endpoints_and_restore_saved_match_owners() {
    let first = ((0usize, usize::MAX), 7usize);
    let second = ((1, 2), usize::MAX);
    let mut node = terminal(vec![second, first]);
    assert!(node.direct_ordinals);
    assert_eq!(node.position(&first), Some(0));
    assert_eq!(node.position(&second), Some(1));
    for unknown in [
        ((0, 0), 7),
        ((0, usize::MAX), 8),
        ((1, 2), 0),
        ((usize::MAX, 2), 7),
    ] {
        assert_eq!(node.position(&unknown), None);
    }
    let arithmetic = NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    };
    let event = Rc::new(TupleChange::put(first, 11_i64)) as Event;
    node.apply(false, &event, arithmetic).unwrap();
    let held = node
        .matches()
        .unwrap()
        .downcast_ref::<SnapshotMap<((usize, usize), usize), (i64, SimpleScore)>>()
        .unwrap()
        .clone();
    let saved = node.begin_checkpoint();
    let bad = Rc::new(TupleChange::put(((0usize, 0usize), 7usize), 99_i64)) as Event;
    assert!(matches!(
        node.apply(false, &bad, arithmetic),
        Err(GraphError::Dependency(_))
    ));
    let changed = Rc::new(TupleChange::put(first, 22_i64)) as Event;
    node.apply(false, &changed, arithmetic).unwrap();
    node.restore(saved);
    assert_eq!(node.total, SimpleScore::new(11));
    assert_eq!(held[&first].0, 11);
    assert_eq!(node.rows.get(0).unwrap().0, 11);
    let sparse = terminal(vec![((2usize, 4usize), 5usize)]);
    assert!(!sparse.direct_ordinals);
    assert_eq!(sparse.position(&((2, 4), 5)), Some(0));
}
struct Custom(usize, Rc<RefCell<Vec<&'static str>>>);
impl Clone for Custom {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push("clone");
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Custom {
    fn eq(&self, other: &Self) -> bool {
        self.1.borrow_mut().push("eq");
        self.0 == other.0
    }
}
impl Eq for Custom {}
impl PartialOrd for Custom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Custom {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.1.borrow_mut().push("ord");
        self.0.cmp(&other.0)
    }
}
#[test]
fn custom_numeric_looking_ids_keep_exact_tree_comparison_and_clone_traces() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let node = terminal(vec![Custom(0, trace.clone()), Custom(1, trace.clone())]);
    assert!(!node.direct_ordinals);
    for id in [0, 1, 2, usize::MAX] {
        let query = Custom(id, trace.clone());
        trace.borrow_mut().clear();
        let expected = node.positions.get(&query).copied();
        let expected_trace = trace.borrow().clone();
        trace.borrow_mut().clear();
        assert_eq!(node.position(&query), expected);
        assert_eq!(*trace.borrow(), expected_trace);
        assert!(!trace.borrow().contains(&"clone"));
        assert!(!trace.borrow().contains(&"eq"));
    }
}

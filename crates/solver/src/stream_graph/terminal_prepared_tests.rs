//! Original prepared-terminal controls; no upstream source-method credit.
use super::*;
use crate::{SimpleScore, primitive_rows::PrimitiveRows};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
type Id = (usize, usize);
type Trace = Vec<(&'static str, i64)>;
type State = Vec<(Id, i64, i64)>;
type Scenario = (Vec<State>, Trace);
thread_local! {
    static TRACE:RefCell<Vec<(&'static str,i64)>>=const {RefCell::new(Vec::new())};
    static FAIL_CLONE:Cell<bool>=const {Cell::new(false)};
}
#[derive(Debug)]
struct Row(i64);
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|t| t.borrow_mut().push(("clone", self.0)));
        assert!(!FAIL_CLONE.with(Cell::get), "payload clone panic");
        Self(self.0)
    }
}
fn node<'a>(
    prepared: bool,
    fail: &'a Cell<bool>,
    structural: &'a Cell<bool>,
) -> Terminal<'a, Id, Row, SimpleScore> {
    Terminal {
        evaluate: TerminalEvaluation::Direct(Box::new(move |row: &Row| {
            TRACE.with(|t| t.borrow_mut().push(("score", row.0)));
            assert!(!fail.get(), "score callback panic");
            SimpleScore::new(row.0).with_structural(if structural.get() { -1 } else { 0 })
        })),
        total: SimpleScore::ZERO,
        name: "sum".into(),
        // Tree storage forces the exact original Terminal::apply body. Numeric
        // key equality has no callbacks, so user payload traces remain comparable.
        matches: if prepared {
            terminal_rows::<Id, Row, SimpleScore>()
        } else {
            PrimitiveRows::new()
        },
        consumed: None,
        checkpoint_box: CheckpointBox::new(),
    }
}
fn apply(
    node: &mut Terminal<'_, Id, Row, SimpleScore>,
    id: Id,
    value: Option<i64>,
) -> Result<(), GraphError> {
    node.apply(
        false,
        &Event::new(TupleChange {
            id,
            value: value.map(Row),
        }),
        NodeArithmetic {
            impact: crate::ScoringArithmetic::Checked,
            totals: crate::ScoringArithmetic::Checked,
        },
    )
    .map(|_| ())
}
fn contents(node: &Terminal<'_, Id, Row, SimpleScore>) -> State {
    node.matches
        .iter()
        .map(|(id, (row, score))| (*id, row.0, score.score()))
        .collect()
}
fn exercise(prepared: bool) -> Scenario {
    TRACE.with(|t| t.borrow_mut().clear());
    let fail = Cell::new(false);
    let structural = Cell::new(false);
    let mut node = node(prepared, &fail, &structural);
    assert_eq!(node.matches.is_primitive(), prepared);
    let ids = [
        (0usize, 0usize),
        (0, 15),
        (0, 16),
        (0, 1024),
        (4095, 1023),
        (4096, 0),
        (usize::MAX, usize::MAX),
    ];
    for (n, id) in ids.into_iter().enumerate() {
        apply(&mut node, id, Some(n as i64)).unwrap();
    }
    let base = contents(&node);
    let full = node.checkpoint();
    let outer = node.begin_checkpoint();
    apply(&mut node, ids[0], Some(30)).unwrap();
    apply(&mut node, ids[1], None).unwrap();
    apply(&mut node, ids[1], Some(-4)).unwrap();
    let inner = node.begin_checkpoint();
    apply(&mut node, ids[2], Some(40)).unwrap();
    apply(&mut node, (usize::MAX, 0usize), Some(50)).unwrap();
    node.commit_checkpoint(inner);
    let wanted = contents(&node);
    let current = node.capture_checkpoint(outer.as_ref());
    assert!(!node.scores_match(outer.as_ref()));
    node.restore(outer);
    assert_eq!(contents(&node), base);
    let other = node.begin_checkpoint();
    apply(&mut node, ids[3], Some(99)).unwrap();
    node.clear();
    node.restore(other);
    assert_eq!(contents(&node), base);
    let original = node.begin_checkpoint();
    let before = TRACE.with(|t| t.borrow().len());
    node.restore(current);
    assert_eq!(contents(&node), wanted);
    node.restore(original);
    assert_eq!(contents(&node), base);
    node.restore(full);
    assert_eq!(TRACE.with(|t| t.borrow().len()), before);
    let pointer = node.matches.get(&ids[0]).unwrap() as *const _;
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| apply(&mut node, ids[0], Some(70)))).is_err());
    fail.set(false);
    assert_eq!(node.matches.get(&ids[0]).unwrap() as *const _, pointer);
    FAIL_CLONE.with(|fail| fail.set(true));
    assert!(catch_unwind(AssertUnwindSafe(|| apply(&mut node, ids[0], Some(80)))).is_err());
    FAIL_CLONE.with(|fail| fail.set(false));
    assert_eq!(node.matches.get(&ids[0]).unwrap() as *const _, pointer);
    structural.set(true);
    assert_eq!(
        apply(&mut node, ids[0], Some(90)),
        Err(GraphError::StructuralContribution("sum".into()))
    );
    structural.set(false);
    assert_eq!(node.matches.get(&ids[0]).unwrap() as *const _, pointer);
    let saved = node.begin_checkpoint();
    node.clear();
    apply(&mut node, ids[0], Some(i64::MAX)).unwrap();
    let before = contents(&node);
    assert_eq!(
        apply(&mut node, ids[1], Some(1)),
        Err(GraphError::ScoreOverflow {
            constraint: Some("sum".into())
        })
    );
    assert_eq!(contents(&node), before);
    node.restore(saved);
    assert_eq!(contents(&node), base);
    let callbacks = TRACE.with(|t| {
        t.borrow()
            .iter()
            .filter(|(kind, _)| *kind == "score")
            .count()
    });
    apply(&mut node, (usize::MAX, 1usize), None).unwrap();
    assert_eq!(
        TRACE.with(|t| t
            .borrow()
            .iter()
            .filter(|(kind, _)| *kind == "score")
            .count()),
        callbacks
    );
    (vec![base, wanted], TRACE.with(|t| t.borrow().clone()))
}
#[test]
fn terminal_prepared_matches_original_clone_callback_error_and_saved_state_contracts() {
    assert_eq!(exercise(true), exercise(false));
}
#[test]
fn terminal_prepared_retains_actual_key_value_owners_and_reinsert_representatives() {
    let fail = Cell::new(false);
    let structural = Cell::new(false);
    let mut node = node(true, &fail, &structural);
    apply(&mut node, (usize::MAX, 1024usize), Some(7)).unwrap();
    let saved = node.begin_checkpoint();
    apply(&mut node, (usize::MAX, 1024usize), Some(8)).unwrap();
    // First public observation after replacement must retain its logical key on
    // rollback, including a current candidate captured over that same base.
    let held = node.matches.materialized().clone();
    let entries = held.shared_entries();
    let key = entries[0].0.clone();
    let value = entries[0].1.clone();
    let captured = node.capture_checkpoint(saved.as_ref());
    node.restore(saved);
    assert!(std::ptr::eq(
        node.matches
            .materialized()
            .get_key_value(&(usize::MAX, 1024))
            .unwrap()
            .0,
        key.as_ref()
    ));
    node.restore(captured);
    assert!(std::ptr::eq(
        node.matches.get(&(usize::MAX, 1024)).unwrap(),
        value.as_ref()
    ));
    let original = node.begin_checkpoint();
    apply(&mut node, (usize::MAX, 1024usize), None).unwrap();
    apply(&mut node, (usize::MAX, 1024usize), Some(9)).unwrap();
    let replaced = node.matches.shared_entries();
    assert!(!Arc::ptr_eq(&replaced[0].0, &key));
    node.restore(original);
    assert!(Arc::ptr_eq(&node.matches.shared_entries()[0].0, &key));
    assert!(Arc::ptr_eq(&node.matches.shared_entries()[0].1, &value));
    let weak = Arc::downgrade(&value);
    drop(entries);
    drop(key);
    drop(value);
    drop(held);
    drop(replaced);
    let saved = node.begin_checkpoint();
    apply(&mut node, (usize::MAX, 1024usize), None).unwrap();
    assert!(weak.upgrade().is_some());
    node.commit_checkpoint(saved);
    assert!(weak.upgrade().is_none());
}
#[test]
fn terminal_prepared_eligibility_excludes_custom_order_and_owned_rows() {
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Custom(usize);
    assert!(prepared_terminal::<Id, Row, SimpleScore>());
    assert!(!prepared_terminal::<Custom, Row, SimpleScore>());
    assert!(!prepared_terminal::<Id, String, SimpleScore>());
    assert!(!prepared_terminal::<Id, Row, crate::SimpleDecimalScore>());
}

#[test]
fn terminal_prepared_equal_write_late_observers_keep_distinct_versions_across_undo_and_install() {
    // A candidate first observed AFTER an equal write must have its own value
    // owner. Both the current capture and full save retain that exact occurrence.
    for prepared in [false, true] {
        let fail = Cell::new(false);
        let structural = Cell::new(false);
        let mut node = node(prepared, &fail, &structural);
        let id = (usize::MAX, usize::MAX);
        apply(&mut node, id, Some(7)).unwrap();
        let original = node.begin_checkpoint();
        apply(&mut node, id, Some(7)).unwrap();
        let candidate = node.matches.materialized().clone();
        let full_candidate = node.checkpoint();
        let current = node.capture_checkpoint(original.as_ref());
        node.restore(original);
        let baseline = node.matches.materialized().clone();
        assert!(!std::ptr::eq(
            candidate.get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
        assert!(std::ptr::eq(
            candidate.get_key_value(&id).unwrap().0,
            baseline.get_key_value(&id).unwrap().0
        ));
        let outer = node.begin_checkpoint();
        node.restore(current);
        assert!(std::ptr::eq(
            node.matches.get(&id).unwrap(),
            candidate.get(&id).unwrap()
        ));
        let nested = node.begin_checkpoint();
        apply(&mut node, id, Some(7)).unwrap();
        let next_candidate = node.matches.materialized().clone();
        assert!(!std::ptr::eq(
            next_candidate.get(&id).unwrap(),
            candidate.get(&id).unwrap()
        ));
        let before = TRACE.with(|trace| trace.borrow().len());
        node.restore(nested);
        assert!(std::ptr::eq(
            node.matches.get(&id).unwrap(),
            candidate.get(&id).unwrap()
        ));
        node.restore(outer);
        assert!(std::ptr::eq(
            node.matches.get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
        let outer = node.begin_checkpoint();
        node.restore(full_candidate);
        assert!(std::ptr::eq(
            node.matches.get(&id).unwrap(),
            candidate.get(&id).unwrap()
        ));
        node.restore(outer);
        assert!(std::ptr::eq(
            node.matches.get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
        assert_eq!(
            TRACE.with(|trace| trace.borrow().len()),
            before,
            "restore must not replay payload Clone or score callbacks"
        );
    }
}

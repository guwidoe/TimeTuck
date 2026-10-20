//! Original terminal storage controls; no upstream method credits.
use super::*;
use crate::{SimpleScore, primitive_rows::PrimitiveRows};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

type Pair = (usize, usize);
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn terminal<'a, I: 'static, T>(
    evaluate: impl Fn(&T) -> SimpleScore + 'a,
    primitive: bool,
) -> Terminal<'a, I, T, SimpleScore> {
    Terminal {
        evaluate: TerminalEvaluation::Direct(Box::new(evaluate)),
        total: SimpleScore::ZERO,
        name: "sum".to_owned(),
        matches: if primitive {
            terminal_rows::<I, T, SimpleScore>()
        } else {
            PrimitiveRows::new()
        },
        consumed: None,
        checkpoint_box: CheckpointBox::new(),
    }
}
fn apply<I: Ord + Clone + 'static, T: Clone + 'static>(
    node: &mut Terminal<'_, I, T, SimpleScore>,
    id: I,
    value: Option<T>,
) -> Result<(), GraphError> {
    let event: Event = Event::new(TupleChange { id, value });
    node.apply(false, &event, arithmetic()).map(|_| ())
}
fn rows(node: &Terminal<'_, Pair, i64, SimpleScore>) -> Vec<(Pair, i64, SimpleScore)> {
    node.matches
        .iter()
        .map(|(id, (row, score))| (*id, *row, *score))
        .collect()
}

#[test]
fn terminal_primitive_sparse_ids_keep_all_matches_and_journal_diagnostics() {
    let mut primitive = terminal::<Pair, _>(|v: &i64| SimpleScore::new(*v), true);
    let mut tree = terminal::<Pair, _>(|v: &i64| SimpleScore::new(*v), false);
    assert!(primitive.matches.is_primitive());
    let ids = [(0, 0), (0, 15), (0, 16), (1, usize::MAX), (usize::MAX, 0)];
    for (n, id) in ids.into_iter().enumerate() {
        apply(&mut primitive, id, Some(n as i64 - 2)).unwrap();
        apply(&mut tree, id, Some(n as i64 - 2)).unwrap();
    }
    let initial = rows(&primitive);
    let old_matches = primitive.matches.materialized().clone();
    let saved = primitive.checkpoint();
    let outer = primitive.begin_checkpoint();
    let tree_outer = tree.begin_checkpoint();
    for (n, id) in ids.into_iter().enumerate() {
        let value = (n % 2 == 0).then_some(n as i64 + 10);
        apply(&mut primitive, id, value).unwrap();
        apply(&mut tree, id, value).unwrap();
    }
    assert_eq!(rows(&primitive), rows(&tree));
    assert_eq!(primitive.score(), tree.score());
    assert!(!primitive.scores_match(outer.as_ref()));
    let inner = primitive.begin_checkpoint();
    apply(&mut primitive, (16, 31), Some(31)).unwrap();
    primitive.commit_checkpoint(inner);
    let changed = rows(&primitive);
    let retained = primitive.checkpoint();
    primitive.restore(outer);
    tree.restore(tree_outer);
    assert_eq!(rows(&primitive), initial);
    assert_eq!(rows(&primitive), rows(&tree));
    let outer = primitive.begin_checkpoint();
    primitive.restore(retained);
    assert_eq!(rows(&primitive), changed);
    primitive.restore(outer);
    assert_eq!(rows(&primitive), initial);
    let outer = primitive.begin_checkpoint();
    primitive.clear();
    assert_eq!(primitive.match_count(), 0);
    primitive.restore(outer);
    assert_eq!(rows(&primitive), initial);
    primitive.restore(saved);
    assert_eq!(primitive.matches.materialized(), &old_matches);
    let published = primitive
        .matches()
        .unwrap()
        .downcast_ref::<SnapshotMap<Pair, (i64, SimpleScore)>>()
        .unwrap();
    assert!(std::ptr::eq(
        published.get(&ids[0]).unwrap(),
        primitive.matches.get(&ids[0]).unwrap()
    ));
}

#[test]
fn terminal_primitive_keeps_callback_and_payload_clone_preflight_failures_atomic() {
    #[derive(Debug)]
    struct Row(i64);
    thread_local! {
        static CLONES: Cell<usize> = const { Cell::new(0) };
        static FAIL: Cell<bool> = const { Cell::new(false) };
    }
    impl Clone for Row {
        fn clone(&self) -> Self {
            CLONES.with(|n| n.set(n.get() + 1));
            assert!(!FAIL.with(Cell::get), "payload clone failure");
            Self(self.0)
        }
    }
    let callback_failure = Cell::new(false);
    let calls = Cell::new(0);
    let mut node = terminal::<Pair, _>(
        |v: &Row| {
            calls.set(calls.get() + 1);
            assert!(!callback_failure.get(), "callback failure");
            SimpleScore::new(v.0)
        },
        true,
    );
    assert!(node.matches.is_primitive());
    CLONES.with(|n| n.set(0));
    apply(&mut node, (1, 16), Some(Row(7))).unwrap();
    assert_eq!(CLONES.with(Cell::get), 1);
    let pointer = node.matches.get(&(1, 16)).unwrap() as *const _;
    let _ = node.matches.materialized();
    let snapshot = node.checkpoint();
    assert_eq!(
        CLONES.with(Cell::get),
        1,
        "inspection and snapshots do not replay user Clone"
    );
    for callback in [true, false] {
        callback_failure.set(callback);
        FAIL.with(|fail| fail.set(!callback));
        assert!(
            catch_unwind(AssertUnwindSafe(|| apply(&mut node, (1, 16), Some(Row(9))))).is_err()
        );
        callback_failure.set(false);
        FAIL.with(|fail| fail.set(false));
        assert_eq!(node.score(), SimpleScore::new(7));
        assert_eq!(node.matches.get(&(1, 16)).unwrap().0.0, 7);
        assert_eq!(node.matches.get(&(1, 16)).unwrap() as *const _, pointer);
    }
    apply(&mut node, (1, 16), None).unwrap();
    assert_eq!(calls.get(), 3, "retraction never evaluates score callback");
    node.restore(snapshot);
    assert_eq!(node.score(), SimpleScore::new(7));
    assert_eq!(node.matches.get(&(1, 16)).unwrap().0.0, 7);
}

#[test]
fn terminal_primitive_structural_and_overflow_errors_preserve_full_payloads() {
    let structural = Cell::new(false);
    let mut node = terminal::<usize, _>(
        |v: &i64| SimpleScore::new(*v).with_structural(if structural.get() { -1 } else { 0 }),
        true,
    );
    apply(&mut node, 0, Some(i64::MAX)).unwrap();
    let before = node.matches.materialized().clone();
    structural.set(true);
    assert!(matches!(
        apply(&mut node, 1, Some(1)),
        Err(GraphError::StructuralContribution(_))
    ));
    structural.set(false);
    assert!(matches!(
        apply(&mut node, 1, Some(1)),
        Err(GraphError::ScoreOverflow { .. })
    ));
    assert_eq!(node.matches.materialized(), &before);
    assert_eq!(node.score(), SimpleScore::new(i64::MAX));
    apply(&mut node, 0, Some(3)).unwrap();
    apply(&mut node, 1, Some(-3)).unwrap();
    let checkpoint = node.begin_checkpoint();
    apply(&mut node, 0, Some(-3)).unwrap();
    apply(&mut node, 1, Some(3)).unwrap();
    assert_eq!(node.score(), SimpleScore::ZERO);
    assert!(
        !node.scores_match(checkpoint.as_ref()),
        "cancelling scores still diagnose changed matches"
    );
    node.restore(checkpoint);
    assert_eq!(node.matches.get(&0).unwrap().0, 3);
}

#[test]
fn terminal_drop_payloads_and_custom_id_types_retain_tree_storage_and_drop_order() {
    #[derive(Clone)]
    struct Row(i64, Rc<RefCell<Vec<i64>>>);
    impl Drop for Row {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn trace(primitive: bool) -> Vec<i64> {
        let drops = Rc::new(RefCell::new(Vec::new()));
        {
            let mut node = terminal::<usize, _>(|v: &Row| SimpleScore::new(v.0), primitive);
            assert!(!node.matches.is_primitive());
            apply(&mut node, 0, Some(Row(1, drops.clone()))).unwrap();
            let checkpoint = node.begin_checkpoint();
            apply(&mut node, 0, Some(Row(2, drops.clone()))).unwrap();
            apply(&mut node, 0, Some(Row(3, drops.clone()))).unwrap();
            node.restore(checkpoint);
        }
        drops.borrow().clone()
    }
    assert_eq!(trace(true), trace(false));
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Id(usize);
    let mut node = terminal::<Id, _>(|v: &i64| SimpleScore::new(*v), true);
    assert!(!node.matches.is_primitive());
    apply(&mut node, Id(1), Some(2)).unwrap();
    assert_eq!(node.score(), SimpleScore::new(2));
}

#[test]
fn terminal_capture_restores_current_sparse_matches_after_other_trials_without_callbacks() {
    let calls = Cell::new(0);
    let mut node = terminal::<Pair, _>(
        |v: &i64| {
            calls.set(calls.get() + 1);
            SimpleScore::new(*v)
        },
        true,
    );
    for id in 0..80 {
        apply(&mut node, (id / 16, id), Some(id as i64)).unwrap();
    }
    let initial = rows(&node);
    let old = node.begin_checkpoint();
    apply(&mut node, (0, 0), None).unwrap();
    apply(&mut node, (0, 15), Some(-5)).unwrap();
    apply(&mut node, (usize::MAX, usize::MAX), Some(70)).unwrap();
    let changed = rows(&node);
    let score = node.score();
    let saved = node.capture_checkpoint(old.as_ref());
    node.restore(old);
    assert_eq!(rows(&node), initial);
    let other = node.begin_checkpoint();
    apply(&mut node, (0, 15), Some(90)).unwrap();
    apply(&mut node, (99, 99), Some(91)).unwrap();
    node.restore(other);
    let outer = node.begin_checkpoint();
    let before = calls.get();
    node.restore(saved);
    assert_eq!(
        calls.get(),
        before,
        "capture installation never replays a callback"
    );
    assert_eq!(rows(&node), changed);
    assert_eq!(node.score(), score);
    assert!(!node.scores_match(outer.as_ref()));
    node.restore(outer);
    assert_eq!(rows(&node), initial);
}

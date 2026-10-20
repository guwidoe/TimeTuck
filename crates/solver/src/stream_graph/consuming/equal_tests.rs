//! Original private equal-contribution controls; no upstream-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

type Id = (usize, usize);
type State = (SimpleScore, Vec<(Id, SimpleScore)>);
fn terminal<'a>(
    private: bool,
    evaluate: impl Fn(&i64) -> SimpleScore + 'a,
) -> Terminal<'a, Id, i64, SimpleScore> {
    let mut node = Terminal {
        evaluate: TerminalEvaluation::Direct(Box::new(evaluate)),
        total: SimpleScore::ZERO,
        name: "sum".into(),
        matches: terminal_rows(),
        consumed: None,
        checkpoint_box: CheckpointBox::new(),
    };
    if private {
        node.enable_consuming_storage();
    }
    node
}
fn apply(
    node: &mut Terminal<'_, Id, i64, SimpleScore>,
    id: Id,
    value: Option<i64>,
    arithmetic: crate::ScoringArithmetic,
) -> Result<(), GraphError> {
    node.apply(
        false,
        &Event::new(TupleChange { id, value }),
        NodeArithmetic {
            impact: arithmetic,
            totals: arithmetic,
        },
    )
    .map(|_| ())
}
fn state(node: &Terminal<'_, Id, i64, SimpleScore>) -> State {
    let rows: Vec<_> = if let Some(rows) = &node.consumed {
        rows.iter().map(|(id, score)| (*id, *score)).collect()
    } else {
        node.matches
            .iter()
            .map(|(id, (_, score))| (*id, *score))
            .collect()
    };
    assert_eq!(node.match_count(), rows.len());
    (node.score(), rows)
}

#[test]
fn consuming_equal_contribution_present_zero_sparse_ids_and_nested_current_full_restore_match_retained()
 {
    fn run(private: bool) -> (Vec<i64>, Vec<State>) {
        let calls = RefCell::new(Vec::new());
        let mut node = terminal(private, |value| {
            calls.borrow_mut().push(*value);
            SimpleScore::new(value % 2)
        });
        let checked = crate::ScoringArithmetic::Checked;
        let ids = [(0, 0), (4095, 1023), (4096, 1024), (usize::MAX, usize::MAX)];
        for (id, value) in ids.into_iter().zip([0, 1, 2, 3]) {
            apply(&mut node, id, Some(value), checked).unwrap();
        }
        let initial = state(&node);
        let full = node.checkpoint();
        let outer = node.begin_checkpoint();
        for id in ids {
            // Changed payloads with the same score must still evaluate.
            apply(&mut node, id, Some(8 + (id.0 % 2) as i64), checked).unwrap();
            apply(&mut node, id, Some(10 + (id.0 % 2) as i64), checked).unwrap();
        }
        let inner = node.begin_checkpoint();
        apply(&mut node, ids[0], None, checked).unwrap();
        assert_eq!(node.match_count(), 3, "a present zero must be removed");
        apply(&mut node, ids[0], None, checked).unwrap();
        apply(&mut node, ids[0], Some(0), checked).unwrap();
        assert_eq!(node.match_count(), 4, "an absent zero must be inserted");
        node.commit_checkpoint(inner);
        let wanted = state(&node);
        let current = node.capture_checkpoint(outer.as_ref());
        let captured_full = node.checkpoint();
        node.restore(outer);
        assert_eq!(state(&node), initial);
        let mut states = vec![initial, wanted.clone()];
        let install = node.begin_checkpoint();
        node.restore(current);
        assert_eq!(state(&node), wanted);
        let calls_before = calls.borrow().len();
        node.restore(install);
        node.restore(captured_full);
        assert_eq!(state(&node), wanted);
        assert_eq!(
            calls.borrow().len(),
            calls_before,
            "restore never evaluates"
        );
        states.push(state(&node));
        node.restore(full);
        let unchanged = node.begin_checkpoint();
        // Same score after prior full installation must preserve diagnostics.
        apply(&mut node, ids[0], Some(100), checked).unwrap();
        assert!(node.scores_match(unchanged.as_ref()));
        let equal_capture = node.capture_checkpoint(unchanged.as_ref());
        node.restore(unchanged);
        let outer = node.begin_checkpoint();
        node.restore(equal_capture);
        states.push(state(&node));
        node.clear();
        assert_eq!(node.match_count(), 0);
        node.restore(outer);
        states.push(state(&node));
        drop(node);
        (calls.into_inner(), states)
    }
    assert_eq!(run(true), run(false));
}

#[test]
fn consuming_equal_contribution_callback_errors_panic_checked_overflow_and_wrapping_match_retained()
{
    fn run(private: bool, arithmetic: crate::ScoringArithmetic) -> (Vec<i64>, Vec<State>) {
        let calls = RefCell::new(Vec::new());
        let mode = Cell::new(0usize);
        let mut node = terminal(private, |value| {
            calls.borrow_mut().push(*value);
            assert_ne!(mode.get(), 1, "callback panic on equal contribution");
            SimpleScore::new(*value).with_structural(i64::from(mode.get() == 2))
        });
        let id = (usize::MAX, 0);
        apply(&mut node, id, Some(i64::MAX), arithmetic).unwrap();
        let initial = state(&node);
        let outer = node.begin_checkpoint();
        apply(&mut node, id, Some(i64::MAX), arithmetic).unwrap();
        assert!(node.scores_match(outer.as_ref()));
        for failure in [1usize, 2] {
            mode.set(failure);
            let result = catch_unwind(AssertUnwindSafe(|| {
                apply(&mut node, id, Some(i64::MAX), arithmetic)
            }));
            if failure == 1 {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(GraphError::StructuralContribution(_))
                ));
            }
            assert_eq!(state(&node), initial);
        }
        mode.set(0);
        let overflow = apply(&mut node, (0, 0), Some(1), arithmetic);
        if arithmetic == crate::ScoringArithmetic::Checked {
            assert!(matches!(overflow, Err(GraphError::ScoreOverflow { .. })));
            assert_eq!(state(&node), initial);
        } else {
            overflow.unwrap();
            assert_eq!(node.score(), SimpleScore::new(i64::MIN));
        }
        let changed = state(&node);
        node.restore(outer);
        assert_eq!(state(&node), initial);
        // A complete checked adjustment permits cancellation of extreme levels.
        apply(&mut node, id, Some(i64::MIN), arithmetic).unwrap();
        apply(&mut node, id, Some(i64::MIN), arithmetic).unwrap();
        let final_state = state(&node);
        drop(node);
        (calls.into_inner(), vec![initial, changed, final_state])
    }
    for arithmetic in [
        crate::ScoringArithmetic::Checked,
        crate::ScoringArithmetic::Wrapping,
    ] {
        assert_eq!(run(true, arithmetic), run(false, arithmetic));
    }
}

#[test]
fn consuming_equal_contribution_graph_callback_counters_capture_and_diagnostics_match_retained() {
    fn run(private: bool) -> (Vec<i64>, Vec<(SimpleScore, u128, usize)>) {
        let calls = RefCell::new(Vec::new());
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, i64>();
        builder
            .score("sum", source.port(), |value| {
                calls.borrow_mut().push(*value);
                SimpleScore::new(value % 2)
            })
            .unwrap();
        let mut graph = builder.build();
        if private {
            assert!(graph.try_enable_consuming_search(source.node_id(), 2));
        }
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, 0i64)).unwrap();
            tx.apply(&source, TupleChange::put(1usize, 1i64)).unwrap();
            tx.commit().unwrap();
        }
        let mut results = Vec::new();
        let saved = {
            let mut tx = graph.transaction();
            for value in [2i64, 4, 6] {
                tx.apply(&source, TupleChange::put(0usize, value)).unwrap();
            }
            results.push((
                tx.score().unwrap(),
                tx.score_evaluations(),
                tx.graph.nodes[tx.graph.constraints["sum"]].match_count(),
            ));
            tx.capture().unwrap()
        };
        let count = calls.borrow().len();
        graph.install(saved);
        assert_eq!(calls.borrow().len(), count);
        graph.assert_consistent().unwrap();
        results.push((
            graph.score(),
            0,
            graph.nodes[graph.constraints["sum"]].match_count(),
        ));
        drop(graph);
        (calls.into_inner(), results)
    }
    assert_eq!(run(true), run(false));
}

#[test]
fn consuming_equal_contribution_public_terminal_keeps_fresh_equal_candidate_owner_versions() {
    let mut node = terminal(false, |_| SimpleScore::ZERO);
    let checked = crate::ScoringArithmetic::Checked;
    let id = (0, 0);
    apply(&mut node, id, Some(7), checked).unwrap();
    let base = node.matches.get(&id).unwrap() as *const (i64, SimpleScore);
    let outer = node.begin_checkpoint();
    apply(&mut node, id, Some(7), checked).unwrap();
    // Future observations AFTER an equal write must still see a fresh owner on
    // public terminals, even when no earlier snapshot forced sharing.
    let candidate = node.matches.materialized().shared_entries().remove(0).1;
    assert_ne!(base, Arc::as_ptr(&candidate));
    let weak = Arc::downgrade(&candidate);
    node.restore(outer);
    assert_eq!(base, node.matches.get(&id).unwrap() as *const _);
    drop(candidate);
    assert!(weak.upgrade().is_none());
    assert_eq!(node.matches.get(&id), Some(&(7, SimpleScore::ZERO)));
}

#[test]
fn consuming_equal_contribution_all_builtin_score_levels_remain_distinct() {
    fn check<S: AdditiveScore + 'static>(scores: &[S]) {
        let selected = Cell::new(0usize);
        let calls = Cell::new(0usize);
        let mut node = Terminal {
            evaluate: TerminalEvaluation::Direct(Box::new(|_: &i64| {
                calls.set(calls.get() + 1);
                scores[selected.get()].clone()
            })),
            total: S::ZERO,
            name: "levels".into(),
            matches: terminal_rows::<Id, i64, S>(),
            consumed: None,
            checkpoint_box: CheckpointBox::new(),
        };
        node.enable_consuming_storage();
        let arithmetic = NodeArithmetic {
            impact: crate::ScoringArithmetic::Checked,
            totals: crate::ScoringArithmetic::Checked,
        };
        let id = (usize::MAX, usize::MAX);
        for (index, expected) in scores.iter().enumerate() {
            selected.set(index);
            for payload in [1i64, 2] {
                node.apply(
                    false,
                    &Event::new(TupleChange::put(id, payload)),
                    arithmetic,
                )
                .unwrap();
                assert_eq!(&node.score(), expected);
                assert_eq!(node.match_count(), 1);
            }
        }
        assert_eq!(calls.get(), scores.len() * 2);
    }
    check(&[
        SimpleScore::new(0),
        SimpleScore::new(1),
        SimpleScore::new(0),
    ]);
    check(&[
        crate::HardSoftScore::new(0, 0),
        crate::HardSoftScore::new(1, 0),
        crate::HardSoftScore::new(1, 1),
        crate::HardSoftScore::new(0, 1),
    ]);
    check(&[
        crate::HardMediumSoftScore::new(0, 0, 0),
        crate::HardMediumSoftScore::new(1, 0, 0),
        crate::HardMediumSoftScore::new(1, 1, 0),
        crate::HardMediumSoftScore::new(1, 1, 1),
        crate::HardMediumSoftScore::new(1, 0, 1),
    ]);
}

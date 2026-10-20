//! Original score-only contracts; no upstream-method credit.
use super::*;
use crate::{ConstraintWeightOverrides, SimpleScore, StreamBatch};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn weights(n: i64) -> ConstraintWeightOverrides<SimpleScore> {
    [("only".to_owned(), s(n))].into_iter().collect()
}
fn unavailable(g: &StreamGraph<'_, SimpleScore>, c: &StreamConstraint<usize, i64>) {
    assert!(matches!(g.constraint_matches(c),
        Err(GraphError::ConstraintMatchesUnavailable(name)) if name == "only"));
    assert!(matches!(g.explain_constraint_default(c, |v| vec![*v]),
        Err(GraphError::ConstraintMatchesUnavailable(name)) if name == "only"));
}

#[test]
fn replacement_retraction_metadata_and_explicit_inspection_error() {
    for pruning in [false, true] {
        let calls = Cell::new(0_u128);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let only = b
            .score_only("only", source.port(), |v| {
                calls.set(calls.get() + 1);
                s(*v)
            })
            .unwrap();
        let tracked = b.score("tracked", source.port(), |v| s(*v)).unwrap();
        let mut g = b.build();
        unavailable(&g, &only); // Empty still means unavailable.
        assert!(g.constraint_matches(&tracked).unwrap().is_empty());
        for (changes, expected, count) in [
            (vec![TupleChange::put(0, 7), TupleChange::put(1, -2)], 5, 2),
            (vec![TupleChange::put(0, 3), TupleChange::retract(1)], 3, 1),
            (vec![TupleChange::retract(99), TupleChange::put(2, 0)], 3, 2),
            (vec![TupleChange::retract(0), TupleChange::retract(2)], 0, 0),
        ] {
            let before = calls.get();
            let mut tx = g.transaction();
            for change in changes {
                tx.apply(&source, change).unwrap();
            }
            // Both terminals perform one physical callback for each observed put.
            assert_eq!(tx.score_evaluations(), 2 * (calls.get() - before));
            assert_eq!(tx.score().unwrap(), s(2 * expected));
            tx.commit().unwrap();
            assert_eq!(g.nodes[only.0.node].match_count(), count);
            assert_eq!(g.nodes[tracked.0.node].match_count(), count);
            assert_eq!(
                g.constraint_scores().collect::<BTreeMap<_, _>>()["only"],
                s(expected)
            );
            assert_eq!(g.constraint_name(&only).unwrap(), "only");
            unavailable(&g, &only);
            g.assert_consistent().unwrap();
        }
    }
}

#[test]
fn batch_nested_diagnostic_rollback_and_saved_candidate_installation() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let positive = b.filter(source.port(), |v| *v >= 0).unwrap();
        let only = b.score_only("only", &positive, |v| s(*v)).unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 2)).unwrap();
            tx.commit().unwrap();
        }
        let saved;
        {
            let mut tx = g.transaction();
            let mut batch = StreamBatch::new();
            batch.push(&source, TupleChange::put(0, -1)).unwrap();
            batch.push(&source, TupleChange::put(1, 9)).unwrap();
            batch.push(&source, TupleChange::put(2, 0)).unwrap();
            tx.apply_batch(batch).unwrap();
            assert_eq!(tx.score().unwrap(), s(9));
            // Diagnostic replay opens nested journals on these same nodes.
            tx.assert_consistent().unwrap();
            saved = tx.capture().unwrap();
            tx.apply(&source, TupleChange::put(1, 70)).unwrap();
            tx.assert_consistent().unwrap();
        }
        assert_eq!(g.score(), s(2));
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
        g.install(saved);
        assert_eq!(g.score(), s(9));
        assert_eq!(g.nodes[only.0.node].match_count(), 2);
        assert_eq!(g.source_rows(&source).unwrap()[&0], -1);
        g.assert_consistent().unwrap();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::retract(1)).unwrap();
            tx.apply(&source, TupleChange::put(0, 4)).unwrap();
            tx.assert_consistent().unwrap();
            assert_eq!(tx.score().unwrap(), s(4));
        }
        assert_eq!(g.score(), s(9));
        assert_eq!(g.nodes[only.0.node].match_count(), 2);
        unavailable(&g, &only);
    }
}

#[test]
fn weighted_disabled_rows_replay_current_sources_and_count_actual_callbacks() {
    for pruning in [false, true] {
        let calls = Cell::new(0_u128);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let c = b
            .score_only_weighted("only", source.port(), s(0), |v| {
                calls.set(calls.get() + 1);
                *v
            })
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 7)).unwrap();
            tx.apply(&source, TupleChange::put(1, 4)).unwrap();
            assert_eq!(tx.score_evaluations(), 0);
            tx.commit().unwrap();
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(g.source_rows(&source).unwrap()[&0], 7);
        assert_eq!(g.nodes[c.0.node].match_count(), 0);
        for (weight, expected) in [(3, 33), (0, 0), (-2, -22), (2, 22)] {
            let before = calls.get();
            let work = g.apply_weight_overrides(&weights(weight)).unwrap();
            assert_eq!(work, calls.get() - before);
            assert_eq!(work, if weight == 0 { 0 } else { 2 });
            assert_eq!(g.score(), s(expected));
            assert_eq!(g.activity().get(&c.0).unwrap().active, weight != 0);
            assert_eq!(
                g.nodes[c.0.node].match_count(),
                if weight == 0 { 0 } else { 2 }
            );
            assert_eq!(g.constraint_weights().next().unwrap(), ("only", &s(weight)));
            assert_eq!(g.apply_weight_overrides(&weights(weight)).unwrap(), 0);
            g.assert_consistent().unwrap();
        }
        g.apply_weight_overrides(&weights(0)).unwrap();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, 12)).unwrap();
            tx.apply(&source, TupleChange::retract(1)).unwrap();
            assert_eq!(tx.score_evaluations(), 0);
            tx.commit().unwrap();
        }
        assert_eq!(g.apply_weight_overrides(&weights(2)).unwrap(), 1);
        assert_eq!(g.score(), s(24));
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
        unavailable(&g, &c);
    }
}

#[test]
fn checked_failure_and_panics_poison_trial_and_restore_every_contribution() {
    let panic_now = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let c = b
        .score_only("only", source.port(), |v| {
            assert!(!panic_now.get());
            if *v == -1 {
                s(0).with_structural(-1)
            } else {
                s(*v)
            }
        })
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, i64::MAX)).unwrap();
        tx.commit().unwrap();
    }
    for structural in [false, true] {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, i64::MAX - 1))
            .unwrap();
        let error = tx.apply(
            &source,
            TupleChange::put(1, if structural { -1 } else { 2 }),
        );
        if structural {
            assert!(matches!(error, Err(GraphError::StructuralContribution(_))));
        } else {
            assert!(
                matches!(error, Err(GraphError::ScoreOverflow { constraint: Some(name) }) if name == "only")
            );
        }
        assert_eq!(tx.score_evaluations(), 2);
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(g.score(), s(i64::MAX));
        assert_eq!(g.nodes[c.0.node].match_count(), 1);
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
    }
    panic_now.set(true);
    {
        let mut tx = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0, 3))
            ))
            .is_err()
        );
        assert_eq!(tx.score_evaluations(), 1);
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    }
    panic_now.set(false);
    assert_eq!(g.score(), s(i64::MAX));
    g.assert_consistent().unwrap();
}

#[test]
fn wrapping_arithmetic_replacement_and_retraction_use_retained_scores() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new()
        .with_arithmetic(crate::ScoringArithmetic::Wrapping);
    let source = b.source::<usize, i64>();
    b.score_only_weighted("only", source.port(), s(2), |v| *v)
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, i64::MAX)).unwrap(); // -2
    tx.apply(&source, TupleChange::put(1, i64::MAX)).unwrap(); // -4
    assert_eq!(tx.score().unwrap(), s(-4));
    tx.apply(&source, TupleChange::put(0, 3)).unwrap(); // 6 + -2
    assert_eq!(tx.score().unwrap(), s(4));
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    assert_eq!(tx.score().unwrap(), s(6));
    assert_eq!(tx.score_evaluations(), 3);
    tx.commit().unwrap();
    g.assert_consistent().unwrap();
}

#[test]
fn failed_weight_override_restores_weights_scores_activity_and_sources() {
    for pruning in [false, true] {
        let panic_now = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, i64>();
        let c = b
            .score_only_weighted("only", source.port(), s(1), |v| {
                assert!(!panic_now.get());
                *v
            })
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, i64::MAX)).unwrap();
            tx.commit().unwrap();
        }
        assert!(matches!(
            g.apply_weight_overrides(&weights(2)),
            Err(GraphError::ScoreOverflow { .. })
        ));
        panic_now.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| g.apply_weight_overrides(&weights(-1)))).is_err());
        panic_now.set(false);
        assert_eq!(g.score(), s(i64::MAX));
        assert_eq!(g.constraint_weights().next().unwrap(), ("only", &s(1)));
        assert_eq!(g.nodes[c.0.node].match_count(), 1);
        assert_eq!(g.source_rows(&source).unwrap()[&0], i64::MAX);
        assert_eq!(g.apply_weight_overrides(&weights(0)).unwrap(), 0);
        assert_eq!(g.apply_weight_overrides(&weights(1)).unwrap(), 1);
        g.assert_consistent().unwrap();
    }
}

#[test]
fn validation_and_foreign_handles_preserve_existing_error_contracts() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let other = foreign.source::<usize, i64>();
    let other_c = foreign.score_only("only", other.port(), |v| s(*v)).unwrap();
    assert!(matches!(
        b.score_only("only", other.port(), |v| s(*v)),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.score_only_weighted("only", source.port(), s(0).with_structural(-1), |_| 1),
        Err(GraphError::StructuralConstraintWeight(_))
    ));
    b.score_only("only", source.port(), |v| s(*v)).unwrap();
    assert!(matches!(
        b.score("only", source.port(), |v| s(*v)),
        Err(GraphError::DuplicateConstraint(_))
    ));
    let mut g = b.build();
    assert!(matches!(
        g.constraint_matches(&other_c),
        Err(GraphError::ForeignPort)
    ));
    assert!(
        matches!(g.apply_weight_overrides(&weights(1)), Err(GraphError::UnweightedConstraintOverride(name)) if name == "only")
    );
}

#[test]
fn diagnostic_detects_cancelling_per_id_corruption_without_payloads() {
    let corrupt = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    b.score_only("only", source.port(), |v| {
        s(if corrupt.get() { 3 - *v } else { *v })
    })
    .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 1)).unwrap();
        tx.apply(&source, TupleChange::put(1, 2)).unwrap();
        tx.commit().unwrap();
    }
    corrupt.set(true);
    let report = g.consistency_report().unwrap();
    assert_eq!(report.working_score, report.recomputed_score);
    assert_eq!(report.nodes.len(), 1); // Per-ID scores changed despite equal total.
    corrupt.set(false);
    g.assert_consistent().unwrap();
}

#[test]
fn terminal_itself_never_clones_or_retains_payload_and_checkpoints_keep_only_scores() {
    struct Payload(Rc<Cell<usize>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    impl Clone for Payload {
        fn clone(&self) -> Self {
            panic!("terminal must not clone this payload")
        }
    }
    let mut node = ScoreOnlyTerminal::<usize, Payload, SimpleScore> {
        evaluate: TerminalEvaluation::Direct(Box::new(|v| s(v.0.get() as i64))),
        total: s(0),
        checkpoint_box: CheckpointBox::new(),
        name: "only".to_owned(),
        contributions: SnapshotMap::new(),
    };
    let drops = Rc::new(Cell::new(8));
    let event = Event::new(TupleChange::put(0_usize, Payload(drops.clone())));
    let arithmetic = NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    };
    node.apply(false, &event, arithmetic).unwrap();
    let saved = node.checkpoint();
    assert!(
        saved
            .downcast_ref::<(SnapshotMap<usize, SimpleScore>, SimpleScore)>()
            .is_some()
    );
    assert_eq!(node.score(), s(8));
    assert_eq!(node.match_count(), 1);
    assert!(node.matches().is_none());
    drop(event);
    assert_eq!(drops.get(), 9); // Neither the terminal nor its snapshot owns T.
    node.clear();
    node.restore(saved);
    assert_eq!(node.score(), s(8));
    assert_eq!(node.match_count(), 1);
}

#[test]
fn metric_capture_reports_true_score_and_zero_contribution_occurrences() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    b.score_only("only", source.port(), |v| s(*v)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.apply(&source, TupleChange::put(1, 0)).unwrap();
    tx.apply(&source, TupleChange::put(2, -2)).unwrap();
    tx.commit().unwrap();
    let capture = crate::solver_metrics::ConstraintMetricCapture::default();
    g.capture_constraint_metrics(&capture);
    let crate::BestConstraintTotals::Available(totals) = capture.snapshot() else {
        panic!("score-only totals remain available");
    };
    assert_eq!(totals["only"].score, s(3));
    assert_eq!(totals["only"].match_count, 3);
}

#[test]
fn decimal_impacts_overrides_and_failed_scale_preserve_exact_state() {
    use crate::{Decimal, DecimalArithmeticError, SimpleDecimalScore};
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let source = b.source::<usize, Decimal>();
    let c = b
        .score_only_weighted_decimal("only", source.port(), "0.1".parse().unwrap(), Clone::clone)
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, "0.025".parse().unwrap()))
            .unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(g.score(), "0.0025".parse().unwrap());
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(1, "0.2".parse().unwrap()))
            .unwrap();
        assert_eq!(
            tx.apply(
                &source,
                TupleChange::put(0, Decimal::new(1.into(), i32::MAX))
            ),
            Err(GraphError::DecimalWeightArithmetic {
                constraint: "only".to_owned(),
                error: DecimalArithmeticError::ScaleOverflow
            })
        );
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        assert_eq!(tx.score_evaluations(), 2);
    }
    assert_eq!(g.score(), "0.0025".parse().unwrap());
    assert_eq!(g.source_rows(&source).unwrap().len(), 1);
    let overrides = [("only".to_owned(), "2.0".parse().unwrap())]
        .into_iter()
        .collect();
    assert_eq!(g.apply_weight_overrides(&overrides).unwrap(), 1);
    assert_eq!(g.score(), "0.0500".parse().unwrap());
    assert!(matches!(
        g.constraint_matches(&c),
        Err(GraphError::ConstraintMatchesUnavailable(_))
    ));
    g.assert_consistent().unwrap();
}

#[test]
fn replay_final_totals_validate_each_terminal_and_whole_graph() {
    for other_terminal in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        b.score_only_weighted("only", source.port(), s(1), |v| *v)
            .unwrap();
        if other_terminal {
            b.score_only("other", source.port(), |_| s(2)).unwrap();
        }
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            // Every new individual impact fits. In the single-terminal case,
            // two impacts overflow the constraint total. With two terminals,
            // only the whole-graph sum overflows after the replay.
            tx.apply(&source, TupleChange::put(0, i64::MAX / 2))
                .unwrap();
            if !other_terminal {
                tx.apply(&source, TupleChange::put(1, i64::MAX / 2))
                    .unwrap();
            }
            tx.commit().unwrap();
        }
        let before = g.score();
        let error = g.apply_weight_overrides(&weights(2));
        assert_eq!(
            error,
            Err(GraphError::ScoreOverflow {
                constraint: if other_terminal {
                    None
                } else {
                    Some("only".to_owned())
                },
            })
        );
        assert_eq!(g.score(), before);
        assert_eq!(g.constraint_weights().next().unwrap(), ("only", &s(1)));
        g.assert_consistent().unwrap();
    }
}

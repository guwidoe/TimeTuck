// Adapted from pinned Timefold Community integer ScoreInliner overflow tests.
// Apache-2.0; exact upstream identities/substitutions: compat/provenance.json.
use std::{fmt::Debug, str::FromStr};
use timetuck::*;
fn overflow<S: AdditiveScore + FromStr + 'static>(weight: &str, total: bool)
where
    S::Err: Debug,
{
    let weight = weight.parse::<S>().unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, i64>();
    b.score_weighted("Test Constraint", source.port(), weight, |v| *v)
        .unwrap();
    let mut g = b.build();
    if total {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 1)).unwrap();
        tx.commit().unwrap();
    }
    let mut tx = g.transaction();
    let error = tx
        .apply(
            &source,
            TupleChange::put(1, if total { 1 } else { i64::MAX }),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        GraphError::ScoreOverflow {
            constraint: Some(_)
        }
    ));
}

#[test]
fn simple_impact_match_weight_overflow() {
    overflow::<SimpleScore>("10", false);
}

#[test]
fn simple_impact_total_overflow() {
    overflow::<SimpleScore>("9223372036854775807", true);
}

#[test]
fn hard_soft_impact_all_match_weight_overflow() {
    overflow::<HardSoftScore>("10hard/100soft", false);
}

#[test]
fn hard_soft_impact_all_total_overflow() {
    overflow::<HardSoftScore>("9223372036854775807hard/9223372036854775807soft", true);
}

#[test]
fn hard_medium_soft_impact_all_match_weight_overflow() {
    overflow::<HardMediumSoftScore>("10hard/100medium/1000soft", false);
}

#[test]
fn hard_medium_soft_impact_all_total_overflow() {
    overflow::<HardMediumSoftScore>(
        "9223372036854775807hard/9223372036854775807medium/9223372036854775807soft",
        true,
    );
}

#[test]
fn bendable_impact_all_match_weight_overflow() {
    overflow::<BendableScore<1, 2>>("[100]hard/[1000/10000]soft", false);
}

#[test]
fn bendable_impact_all_total_overflow() {
    overflow::<BendableScore<1, 2>>(
        "[9223372036854775807]hard/[9223372036854775807/9223372036854775807]soft",
        true,
    );
}

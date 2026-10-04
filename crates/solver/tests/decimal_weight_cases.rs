// Adapted from pinned Timefold Community decimal ScoreInliner tests.
// Apache-2.0; exact source identities and substitutions: compat/provenance.json.
use std::{fmt::Debug, str::FromStr};
use timetuck::*;

fn default_score<S: DecimalWeightedScore + 'static>() {
    let graph = StreamGraphBuilder::<S>::new().build();
    assert_eq!(graph.score(), S::ZERO);
}
fn impact<S: DecimalWeightedScore + FromStr + 'static>(
    weight: &str,
    factors: [&str; 2],
    expected: [&str; 4],
) where
    S::Err: Debug,
{
    let mut builder = StreamGraphBuilder::<S>::new();
    let source = builder.source::<usize, Decimal>();
    builder
        .score_weighted_decimal(
            "Test Constraint",
            source.port(),
            weight.parse().unwrap(),
            Clone::clone,
        )
        .unwrap();
    let mut graph = builder.build();
    for (change, expected) in [
        (
            TupleChange::put(0, factors[0].parse().unwrap()),
            expected[0],
        ),
        (
            TupleChange::put(1, factors[1].parse().unwrap()),
            expected[1],
        ),
        (TupleChange::retract(1), expected[2]),
        (TupleChange::retract(0), expected[3]),
    ] {
        let mut tx = graph.transaction();
        tx.apply(&source, change).unwrap();
        tx.commit().unwrap();
        assert_eq!(graph.score(), expected.parse::<S>().unwrap());
    }
}

#[test]
fn simple_default_score() {
    default_score::<SimpleDecimalScore>();
}

#[test]
fn simple_impact() {
    impact::<SimpleDecimalScore>("10", ["10", "20"], ["100", "300", "100", "0"]);
}

#[test]
fn hard_soft_default_score() {
    default_score::<HardSoftDecimalScore>();
}

#[test]
fn hard_soft_impact_hard() {
    impact::<HardSoftDecimalScore>(
        "90hard/0soft",
        ["1", "2"],
        [
            "90hard/0soft",
            "270hard/0soft",
            "90hard/0soft",
            "0hard/0soft",
        ],
    );
}

#[test]
fn hard_soft_impact_soft() {
    impact::<HardSoftDecimalScore>(
        "0hard/90soft",
        ["1", "2"],
        [
            "0hard/90soft",
            "0hard/270soft",
            "0hard/90soft",
            "0hard/0soft",
        ],
    );
}

#[test]
fn hard_soft_impact_all() {
    impact::<HardSoftDecimalScore>(
        "10hard/100soft",
        ["10", "20"],
        [
            "100hard/1000soft",
            "300hard/3000soft",
            "100hard/1000soft",
            "0hard/0soft",
        ],
    );
}

#[test]
fn hard_medium_soft_default_score() {
    default_score::<HardMediumSoftDecimalScore>();
}

#[test]
fn hard_medium_soft_impact_hard() {
    impact::<HardMediumSoftDecimalScore>(
        "90hard/0medium/0soft",
        ["1", "2"],
        [
            "90hard/0medium/0soft",
            "270hard/0medium/0soft",
            "90hard/0medium/0soft",
            "0hard/0medium/0soft",
        ],
    );
}

#[test]
fn hard_medium_soft_impact_medium() {
    impact::<HardMediumSoftDecimalScore>(
        "0hard/90medium/0soft",
        ["1", "2"],
        [
            "0hard/90medium/0soft",
            "0hard/270medium/0soft",
            "0hard/90medium/0soft",
            "0hard/0medium/0soft",
        ],
    );
}

#[test]
fn hard_medium_soft_impact_soft() {
    impact::<HardMediumSoftDecimalScore>(
        "0hard/0medium/90soft",
        ["1", "2"],
        [
            "0hard/0medium/90soft",
            "0hard/0medium/270soft",
            "0hard/0medium/90soft",
            "0hard/0medium/0soft",
        ],
    );
}

#[test]
fn hard_medium_soft_impact_all() {
    impact::<HardMediumSoftDecimalScore>(
        "10hard/100medium/1000soft",
        ["10", "20"],
        [
            "100hard/1000medium/10000soft",
            "300hard/3000medium/30000soft",
            "100hard/1000medium/10000soft",
            "0hard/0medium/0soft",
        ],
    );
}

#[test]
fn bendable_default_score() {
    default_score::<BendableDecimalScore<1, 2>>();
}

#[test]
fn bendable_impact_hard() {
    impact::<BendableDecimalScore<1, 2>>(
        "[90]hard/[0/0]soft",
        ["1", "2"],
        [
            "[90]hard/[0/0]soft",
            "[270]hard/[0/0]soft",
            "[90]hard/[0/0]soft",
            "[0]hard/[0/0]soft",
        ],
    );
}

#[test]
fn bendable_impact_soft1() {
    impact::<BendableDecimalScore<1, 2>>(
        "[0]hard/[90/0]soft",
        ["1", "2"],
        [
            "[0]hard/[90/0]soft",
            "[0]hard/[270/0]soft",
            "[0]hard/[90/0]soft",
            "[0]hard/[0/0]soft",
        ],
    );
}

#[test]
fn bendable_impact_soft2() {
    impact::<BendableDecimalScore<1, 2>>(
        "[0]hard/[0/90]soft",
        ["1", "2"],
        [
            "[0]hard/[0/90]soft",
            "[0]hard/[0/270]soft",
            "[0]hard/[0/90]soft",
            "[0]hard/[0/0]soft",
        ],
    );
}

#[test]
fn bendable_impact_all() {
    impact::<BendableDecimalScore<1, 2>>(
        "[10]hard/[100/1000]soft",
        ["10", "20"],
        [
            "[100]hard/[1000/10000]soft",
            "[300]hard/[3000/30000]soft",
            "[100]hard/[1000/10000]soft",
            "[0]hard/[0/0]soft",
        ],
    );
}

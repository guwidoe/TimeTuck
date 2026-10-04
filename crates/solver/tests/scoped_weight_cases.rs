// Adapted from pinned Timefold Community integer ScoreInliner tests.
// Apache-2.0; exact identities and substitutions: compat/provenance.json.
use std::{fmt::Debug, str::FromStr};
use timetuck::*;
fn default_score<S: AdditiveScore>() {
    let problem = FiniteProblem::<i64>::new(vec![]).unwrap();
    let network = ConstraintNetwork::<i64, S>::new(0);
    assert_eq!(
        network
            .calculate(&problem, &problem.initial_assignment())
            .unwrap(),
        S::ZERO
    );
}
fn impact<S: AdditiveScore + FromStr>(weight: &str, factors: [i64; 2], expected: [&str; 4])
where
    S::Err: Debug,
{
    let problem = FiniteProblem::new(
        factors
            .iter()
            .map(|&v| Variable::new(vec![v]).allowing_unassigned())
            .collect(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(2);
    network
        .add_weighted_constraint(
            "Test Constraint",
            vec![0, 1],
            weight.parse::<S>().unwrap(),
            |values| values.iter().filter_map(|v| v.copied()).sum(),
        )
        .unwrap();
    let mut director =
        ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    for (variable, value, expected) in [
        (0, Some(factors[0]), expected[0]),
        (1, Some(factors[1]), expected[1]),
        (1, None, expected[2]),
        (0, None, expected[3]),
    ] {
        director
            .apply(&Move::Change { variable, value })
            .unwrap()
            .commit();
        assert_eq!(director.score(), expected.parse::<S>().unwrap());
    }
}

#[test]
fn simple_default_score() {
    default_score::<SimpleScore>();
}

#[test]
fn simple_impact() {
    impact::<SimpleScore>("10", [10, 20], ["100", "300", "100", "0"]);
}

#[test]
fn hard_soft_default_score() {
    default_score::<HardSoftScore>();
}

#[test]
fn hard_soft_impact_hard() {
    impact::<HardSoftScore>(
        "90hard/0soft",
        [1, 2],
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
    impact::<HardSoftScore>(
        "0hard/90soft",
        [1, 2],
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
    impact::<HardSoftScore>(
        "10hard/100soft",
        [10, 20],
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
    default_score::<HardMediumSoftScore>();
}

#[test]
fn hard_medium_soft_impact_hard() {
    impact::<HardMediumSoftScore>(
        "90hard/0medium/0soft",
        [1, 2],
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
    impact::<HardMediumSoftScore>(
        "0hard/90medium/0soft",
        [1, 2],
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
    impact::<HardMediumSoftScore>(
        "0hard/0medium/90soft",
        [1, 2],
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
    impact::<HardMediumSoftScore>(
        "10hard/100medium/1000soft",
        [10, 20],
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
    default_score::<BendableScore<1, 2>>();
}

#[test]
fn bendable_impact_hard() {
    impact::<BendableScore<1, 2>>(
        "[90]hard/[0/0]soft",
        [1, 2],
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
    impact::<BendableScore<1, 2>>(
        "[0]hard/[90/0]soft",
        [1, 2],
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
    impact::<BendableScore<1, 2>>(
        "[0]hard/[0/90]soft",
        [1, 2],
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
    impact::<BendableScore<1, 2>>(
        "[10]hard/[100/1000]soft",
        [10, 20],
        [
            "[100]hard/[1000/10000]soft",
            "[300]hard/[3000/30000]soft",
            "[100]hard/[1000/10000]soft",
            "[0]hard/[0/0]soft",
        ],
    );
}

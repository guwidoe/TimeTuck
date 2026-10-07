// Adapted assertion matrices from pinned SingleConstraintAssertionTest.
// Apache-2.0; see compat/provenance.json.
use super::*;
#[test]
fn penalizes_and_does_not_reward() {
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(0)),
        Some("There should be penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(0)),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward"],
    );
}
#[test]
fn rewards_but_does_not_penalize() {
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(0)),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(0)),
        Some("There should be penalties"),
        false,
        &["There should be penalties", "Expected penalty"],
    );
}
#[test]
fn has_no_impact() {
    scalar_case(
        Mode::Penalty,
        Given::Empty,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        true,
        &[],
    );
    scalar_case(
        Mode::Penalty,
        Given::Empty,
        ConstraintExpectation::NoImpact,
        None,
        true,
        &[],
    );
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        false,
        &[
            "There should be no impact",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::NoImpact,
        None,
        false,
        &[
            "Broken expectation",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        false,
        &[
            "There should be no impact",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::NoImpact,
        None,
        false,
        &[
            "Broken expectation",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
}
#[test]
fn has_no_impact_with_mixed_constraint() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::NoImpact,
        None,
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        false,
        &[
            "There should be no impact",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::NoImpact,
        None,
        false,
        &[
            "Broken expectation",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::NoImpact,
        Some("There should be no impact"),
        false,
        &[
            "There should be no impact",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::NoImpact,
        None,
        false,
        &[
            "Broken expectation",
            "Constraint",
            "Expected",
            "no impact",
            "Actual impact",
        ],
    );
}
#[test]
fn impacts() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(0)),
        Some("There should be no rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(0)),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(1)),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(2)),
        Some("There should only be one reward"),
        false,
        &["There should only be one reward", "Expected reward"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(1)),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(0)),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(0)),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(1)),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(2)),
        Some("There should only be one penalty"),
        false,
        &["There should only be one penalty", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(1)),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
}
#[test]
fn impacts_more_than() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(0)),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(0)),
        Some("There should be no rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(1)),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(2)),
        Some("There should only be one reward"),
        false,
        &["There should only be one reward", "Expected reward"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(1)),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(0)),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(0)),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(1)),
        Some("There should be penalties"),
        false,
        &["There should be penalties", "Expected penalty more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::MoreThan(2)),
        Some("There should only be one penalty"),
        false,
        &["There should only be one penalty", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardCount(ConstraintComparison::MoreThan(1)),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
}
#[test]
fn impacts_less_than() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardCount(ConstraintComparison::LessThan(1)),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::LessThan(1)),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward less than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::LessThan(2)),
        Some("There should be less than 2 rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardCount(ConstraintComparison::LessThan(3)),
        Some("There be less than 3 rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::LessThan(2)),
        Some("There should be 1 reward"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::LessThan(1)),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::LessThan(1)),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty less than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::LessThan(2)),
        Some("There should be less than 2 penalities"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::LessThan(3)),
        Some("There should be less than 2 penalities"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardCount(ConstraintComparison::LessThan(2)),
        Some("There should be no rewards"),
        true,
        &[],
    );
}
#[test]
fn impacts_by() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("0".parse().unwrap())),
        Some("There should be no rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("0".parse().unwrap())),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("1".parse().unwrap())),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("2".parse().unwrap())),
        Some("There should only be one reward"),
        false,
        &["There should only be one reward", "Expected reward"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("1".parse().unwrap())),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("0".parse().unwrap())),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("0".parse().unwrap())),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("1".parse().unwrap())),
        Some("There should be penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("2".parse().unwrap())),
        Some("There should only be one penalty"),
        false,
        &["There should only be one penalty", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("1".parse().unwrap())),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
}
#[test]
fn impacts_by_more_than() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::MoreThan("0".parse().unwrap())),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::MoreThan("0".parse().unwrap())),
        Some("There should be no rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::MoreThan("1".parse().unwrap())),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::MoreThan("2".parse().unwrap())),
        Some("There should only be one reward"),
        false,
        &[
            "There should only be one reward",
            "Expected reward more than",
        ],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::MoreThan("1".parse().unwrap())),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::MoreThan("0".parse().unwrap())),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::MoreThan("0".parse().unwrap())),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::MoreThan("1".parse().unwrap())),
        Some("There should be penalties"),
        false,
        &["There should be penalties", "Expected penalty more than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::MoreThan("2".parse().unwrap())),
        Some("There should only be one penalty"),
        false,
        &["There should only be one penalty", "Expected penalty"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::MoreThan("1".parse().unwrap())),
        Some("There should be no rewards"),
        false,
        &["There should be no rewards", "Expected reward"],
    );
}
#[test]
fn impacts_by_less_than() {
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::LessThan("1".parse().unwrap())),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::LessThan("1".parse().unwrap())),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward less than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::LessThan("2".parse().unwrap())),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::RewardImpact(ConstraintComparison::LessThan("3".parse().unwrap())),
        Some("There should only be one reward"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Reward,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::LessThan("2".parse().unwrap())),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Empty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::LessThan("1".parse().unwrap())),
        Some("There should be no penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::LessThan("1".parse().unwrap())),
        Some("There should be no penalties"),
        false,
        &["There should be no penalties", "Expected penalty less than"],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::LessThan("2".parse().unwrap())),
        Some("There should be penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::LessThan("3".parse().unwrap())),
        Some("There should only be one penalty"),
        true,
        &[],
    );
    scalar_case(
        Mode::Mixed,
        Given::Penalty,
        ConstraintExpectation::RewardImpact(ConstraintComparison::LessThan("2".parse().unwrap())),
        Some("There should be no rewards"),
        true,
        &[],
    );
}
#[test]
fn penalizes_by_count_and_does_not_reward() {
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(3)),
        Some("There should be penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(1)),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward"],
    );
}
#[test]
fn penalizes_by_big_decimal() {
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly("3".parse().unwrap())),
        Some("There should be penalties"),
        true,
        &[],
    );
    scalar_case(
        Mode::Penalty,
        Given::Generated,
        ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly(
            "3.01".parse().unwrap(),
        )),
        Some("There should be penalties"),
        false,
        &["There should be penalties", "Expected penalty"],
    );
}
#[test]
fn rewards_by_count_but_does_not_penalize() {
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(3)),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(1)),
        Some("There should be penalties"),
        false,
        &["There should be penalties", "Expected penalty"],
    );
}
#[test]
fn rewards_by_big_decimal() {
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("3".parse().unwrap())),
        Some("There should be rewards"),
        true,
        &[],
    );
    scalar_case(
        Mode::Reward,
        Given::Generated,
        ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly("3.01".parse().unwrap())),
        Some("There should be rewards"),
        false,
        &["There should be rewards", "Expected reward"],
    );
}

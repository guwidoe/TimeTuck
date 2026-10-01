// Adapted from Timefold Community v2.7.0, core/src/test/java/ai/timefold/solver/core/api/score/BendableScoreTest.java.
// Commit: 8b815bc6b5f3eb81feedc94dea0885a93b267762; Apache-2.0. See THIRD_PARTY_NOTICES.md.
// Generated from tests only by scripts/adapt_bendable_tests.py.
// Compile-time shapes replace score definitions. Assertions and values preserved.
mod common;
use common::{distinct, equal, ordered};
use std::ops::{Add, Neg, Sub};
use timetuck::BendableScore;
type Hss = BendableScore<1, 2>;
type Hhsss = BendableScore<2, 3>;
type Hhs = BendableScore<2, 1>;
type Empty = BendableScore<0, 0>;
type Soft = BendableScore<0, 1>;

#[test]
fn of() {
    assert_eq!(Hss::of_hard(0, -147).unwrap(), Hss::new([-147], [0, 0]));
    assert_eq!(Hss::of_soft(0, -258).unwrap(), Hss::new([0], [-258, 0]));
    assert_eq!(Hss::of_soft(1, -369).unwrap(), Hss::new([0], [0, -369]));
}

#[test]
fn parse_score() {
    assert_eq!(
        "[-5432109876]hard/[-9876543210/-3456789012]soft"
            .parse::<Hss>()
            .unwrap(),
        Hss::new([-5432109876], [-9876543210, -3456789012])
    );
    assert_eq!(
        "[-5432109876]hard/[-9876543210/*]soft"
            .parse::<Hss>()
            .unwrap(),
        Hss::new([-5432109876], [-9876543210, i64::MIN])
    );
    assert_eq!(
        "[-5432109876]hard/[*/-3456789012]soft"
            .parse::<Hss>()
            .unwrap(),
        Hss::new([-5432109876], [i64::MIN, -3456789012])
    );
    assert_eq!(
        "-1structural/[-5432109876]hard/[-9876543210/-3456789012]soft"
            .parse::<Hss>()
            .unwrap(),
        Hss::new([-5432109876], [-9876543210, -3456789012]).with_structural(-1)
    );
}

#[test]
fn to_short_string() {
    assert_eq!(
        Hss::new([0], [0, -3456789012]).to_short_string(),
        "[0/-3456789012]soft"
    );
    assert_eq!(
        Hss::new([0], [-9876543210, -3456789012]).to_short_string(),
        "[-9876543210/-3456789012]soft"
    );
    assert_eq!(
        Hss::new([-5432109876], [0, -0]).to_short_string(),
        "[-5432109876]hard"
    );
    assert_eq!(
        Hss::new([-5432109876], [-9876543210, -3456789012]).to_short_string(),
        "[-5432109876]hard/[-9876543210/-3456789012]soft"
    );
    assert_eq!(
        Hss::new([-5432109876], [-9876543210, -3456789012])
            .with_structural(-1)
            .to_short_string(),
        "-1structural/[-5432109876]hard/[-9876543210/-3456789012]soft"
    );
}

#[test]
fn test_to_string() {
    assert_eq!(
        (Hss::new([0], [-9876543210, -3456789012])).to_string(),
        "[0]hard/[-9876543210/-3456789012]soft"
    );
    assert_eq!(
        (Hss::new([-5432109876], [-9876543210, -3456789012])).to_string(),
        "[-5432109876]hard/[-9876543210/-3456789012]soft"
    );
    assert_eq!(
        (Hhs::new([-5432109876, -9876543210], [-3456789012])).to_string(),
        "[-5432109876/-9876543210]hard/[-3456789012]soft"
    );
    assert_eq!((Empty::new([], [])).to_string(), "[]hard/[]soft");
    assert_eq!(
        (Hss::new([-5432109876], [-9876543210, -3456789012]).with_structural(-1)).to_string(),
        "-1structural/[-5432109876]hard/[-9876543210/-3456789012]soft"
    );
}

#[test]
fn parse_score_illegal_argument() {
    assert!("-147".parse::<Hss>().is_err());
    assert!(
        "-1structural/[-5432109876]hard/[-9876543210]soft"
            .parse::<Hss>()
            .is_err()
    );
}

#[test]
fn get_hard_or_soft_score() {
    let score = Hss::new([-5], [-10, -200]);
    assert_eq!(score.level(0), Some(-5));
    assert_eq!(score.level(1), Some(-10));
    assert_eq!(score.level(2), Some(-200));
}

#[test]
fn feasible_h_s_s() {
    assert!(!(Hss::new([-5], [-300, -9876543210])).is_feasible());
    assert!(!(Hss::new([0], [-300, -9876543210]).with_structural(-1)).is_feasible());
    assert!((Hss::new([0], [-300, -9876543210])).is_feasible());
    assert!((Hss::new([2], [-300, -9876543210])).is_feasible());
}

#[test]
fn add_h_s_s() {
    assert_eq!(
        Hss::new([1111111111], [-20, -9876543210]).add(Hss::new([2222222222], [-300, 9876543210])),
        Hss::new([3333333333], [-320, 0])
    );
}

#[test]
fn subtract_h_s_s() {
    assert_eq!(
        Hss::new([3333333333], [-20, -5555555555]).sub(Hss::new([1111111111], [-300, 3333333333])),
        Hss::new([2222222222], [280, -8888888888])
    );
}

#[test]
fn multiply_h_s_s() {
    assert_eq!(
        Hss::new([5000000000], [-5000000000, 5000000000]).multiply(1.2),
        Hss::new([6000000000], [-6000000000, 6000000000])
    );
    assert_eq!(Hss::new([1], [-1, 1]).multiply(1.2), Hss::new([1], [-2, 1]));
    assert_eq!(Hss::new([4], [-4, 4]).multiply(1.2), Hss::new([4], [-5, 4]));
}

#[test]
fn divide_h_s_s() {
    assert_eq!(
        Hss::new([25000000000], [-25000000000, 25000000000]).divide(5.0),
        Hss::new([5000000000], [-5000000000, 5000000000])
    );
    assert_eq!(
        Hss::new([21], [-21, 21]).divide(5.0),
        Hss::new([4], [-5, 4])
    );
    assert_eq!(
        Hss::new([24], [-24, 24]).divide(5.0),
        Hss::new([4], [-5, 4])
    );
}

#[test]
fn power_h_s_s() {
    assert_eq!(
        Hss::new([300000], [-400000, 500000]).power(2.0),
        Hss::new([90000000000], [160000000000, 250000000000])
    );
    assert_eq!(
        Hss::new([90000000000], [160000000000, 250000000000]).power(0.5),
        Hss::new([300000], [400000, 500000])
    );
}

#[test]
fn negate_h_s_s() {
    assert_eq!(
        Hss::new([3000000000], [-4000000000, 5000000000]).neg(),
        Hss::new([-3000000000], [4000000000, -5000000000])
    );
    assert_eq!(
        Hss::new([-3000000000], [4000000000, -5000000000]).neg(),
        Hss::new([3000000000], [-4000000000, 5000000000])
    );
}

#[test]
fn abs_h_s_s() {
    assert_eq!(
        Hss::new([3000000000], [4000000000, 5000000000]).abs(),
        Hss::new([3000000000], [4000000000, 5000000000])
    );
    assert_eq!(
        Hss::new([3000000000], [-4000000000, 5000000000]).abs(),
        Hss::new([3000000000], [4000000000, 5000000000])
    );
    assert_eq!(
        Hss::new([-3000000000], [4000000000, -5000000000]).abs(),
        Hss::new([3000000000], [4000000000, 5000000000])
    );
    assert_eq!(
        Hss::new([-3000000000], [-4000000000, -5000000000]).abs(),
        Hss::new([3000000000], [4000000000, 5000000000])
    );
}

#[test]
fn zero() {
    assert_eq!(Soft::ZERO, Soft::new([], [0]));
    assert!(Soft::ZERO.is_zero());
    assert!(!Soft::of_soft(0, 1).unwrap().is_zero());
}

#[test]
fn equals_and_hash_code_h_s_s() {
    equal(&[
        Hss::new([-10], [-200, -3000]),
        Hss::new([-10], [-200, -3000]),
    ]);
    equal(&[
        Hss::new([-10], [-200, -3000]).with_structural(-1),
        Hss::new([-10], [-200, -3000]).with_structural(-1),
    ]);
    distinct(&[
        Hss::new([-10], [-200, -3000]),
        Hss::new([-30], [-200, -3000]),
        Hss::new([-10], [-400, -3000]),
        Hss::new([-10], [-400, -5000]),
        Hss::new([-10], [-200, -3000]).with_structural(-1),
    ]);
}

#[test]
fn compare_to_h_s_s() {
    ordered(&[
        Hss::new([-20], [i64::MIN, i64::MIN]).with_structural(-1),
        Hss::new([-20], [i64::MIN, i64::MIN]),
        Hss::new([-20], [i64::MIN, -20]),
        Hss::new([-20], [i64::MIN, 1]),
        Hss::new([-20], [-300, -4000]),
        Hss::new([-20], [-300, -300]),
        Hss::new([-20], [-300, -20]),
        Hss::new([-20], [-300, 300]),
        Hss::new([-20], [-20, -300]),
        Hss::new([-20], [-20, 0]),
        Hss::new([-20], [-20, 1]),
        Hss::new([-1], [-300, -4000]),
        Hss::new([-1], [-300, -20]),
        Hss::new([-1], [-20, -300]),
        Hss::new([1], [i64::MIN, -20]),
        Hss::new([1], [-20, i64::MIN]),
    ]);
}

#[test]
fn feasible_h_h_s_s_s() {
    assert!(!(Hhsss::new([-5, 0], [-300, -4000000000, -5000])).is_feasible());
    assert!(!(Hhsss::new([0, -5000000000], [-300, -4000, -5000])).is_feasible());
    assert!(!(Hhsss::new([1, -2], [-300, -4000, -5000])).is_feasible());
    assert!((Hhsss::new([0, 0], [-300000000000, -4000, -5000])).is_feasible());
    assert!((Hhsss::new([0, 2], [-300, -4000, -50000000000])).is_feasible());
    assert!((Hhsss::new([2000000000, 0], [-300, -4000, -5000])).is_feasible());
    assert!((Hhsss::new([1, 2], [-300, -4000, -5000])).is_feasible());
}

#[test]
fn add_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([20000000000, -20000000000], [-4000000000000, 0, 0]).add(Hhsss::new(
            [-1000000000, -300000000000],
            [4000000000000, 0, 0]
        )),
        Hhsss::new([19000000000, -320000000000], [0, 0, 0])
    );
}

#[test]
fn subtract_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([20000000000, -20000000000], [-4000000000000, 0, 0]).sub(Hhsss::new(
            [-1000000000, -300000000000],
            [4000000000000, 0, 0]
        )),
        Hhsss::new([21000000000, 280000000000], [-8000000000000, 0, 0])
    );
}

#[test]
fn multiply_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([5000000000, -5000000000], [5000000000, 0, 0]).multiply(1.2),
        Hhsss::new([6000000000, -6000000000], [6000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([1, -1], [1, 0, 0]).multiply(1.2),
        Hhsss::new([1, -2], [1, 0, 0])
    );
    assert_eq!(
        Hhsss::new([4, -4], [4, 0, 0]).multiply(1.2),
        Hhsss::new([4, -5], [4, 0, 0])
    );
}

#[test]
fn divide_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([25000000000, -25000000000], [25000000000, 0, 0]).divide(5.0),
        Hhsss::new([5000000000, -5000000000], [5000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([21, -21], [21, 0, 0]).divide(5.0),
        Hhsss::new([4, -5], [4, 0, 0])
    );
    assert_eq!(
        Hhsss::new([24, -24], [24, 0, 0]).divide(5.0),
        Hhsss::new([4, -5], [4, 0, 0])
    );
}

#[test]
fn power_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([200000, -400000], [500000, 0, 0]).power(2.0),
        Hhsss::new([40000000000, 160000000000], [250000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([9, 16], [25, 0, 0]).power(0.5),
        Hhsss::new([3, 4], [5, 0, 0])
    );
}

#[test]
fn negate_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([3000000000, -4000000000], [5000000000, 0, 0]).neg(),
        Hhsss::new([-3000000000, 4000000000], [-5000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([-3, 4], [-5, 0, 0]).neg(),
        Hhsss::new([3, -4], [5, 0, 0])
    );
}

#[test]
fn abs_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([3000000000, 4000000000], [5000000000, 0, 0]).abs(),
        Hhsss::new([3000000000, 4000000000], [5000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([3000000000, -4000000000], [5000000000, 0, 0]).abs(),
        Hhsss::new([3000000000, 4000000000], [5000000000, 0, 0])
    );
    assert_eq!(
        Hhsss::new([-3, 4], [-5, 0, 0]).abs(),
        Hhsss::new([3, 4], [5, 0, 0])
    );
    assert_eq!(
        Hhsss::new([-3, -4], [-5, 0, 0]).abs(),
        Hhsss::new([3, 4], [5, 0, 0])
    );
}

#[test]
fn equals_and_hash_code_h_h_s_s_s() {
    equal(&[
        Hhsss::new([-10000000000, -20000000000], [-30000000000, 0, 0]),
        Hhsss::new([-10000000000, -20000000000], [-30000000000, 0, 0]),
    ]);
}

#[test]
fn compare_to_h_h_s_s_s() {
    ordered(&[
        Hhsss::new([-20, i64::MIN], [i64::MIN, 0, 0]),
        Hhsss::new([-20, i64::MIN], [-20, 0, 0]),
        Hhsss::new([-20, i64::MIN], [1, 0, 0]),
        Hhsss::new([-20, -300], [-4000, 0, 0]),
        Hhsss::new([-20, -300], [-300, 0, 0]),
        Hhsss::new([-20, -300], [-20, 0, 0]),
        Hhsss::new([-20, -300], [300, 0, 0]),
        Hhsss::new([-20, -20], [-300, 0, 0]),
        Hhsss::new([-20, -20], [0, 0, 0]),
        Hhsss::new([-20, -20], [1, 0, 0]),
        Hhsss::new([-1, -300], [-4000, 0, 0]),
        Hhsss::new([-1, -300], [-20, 0, 0]),
        Hhsss::new([-1, -20], [-300, 0, 0]),
        Hhsss::new([1, i64::MIN], [-20, 0, 0]),
        Hhsss::new([1, -20], [i64::MIN, 0, 0]),
    ]);
}

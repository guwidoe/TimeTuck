// Adapted from Timefold Community v2.7.0, core/src/test/java/ai/timefold/solver/core/api/score/BendableBigDecimalScoreTest.java.
// Commit: 8b815bc6b5f3eb81feedc94dea0885a93b267762; Apache-2.0. See THIRD_PARTY_NOTICES.md.
// Generated from tests only by scripts/adapt_bendable_decimal_tests.py.
// Native const dimensions replace definitions; all assertions/values preserved.
// Decimal factors, u32 powers, Result/Option, ZERO and Rust Eq/Ord/Hash helpers.
mod common;
use common::{distinct, equal, ordered};
use std::ops::{Add, Neg, Sub};
use timetuck::{BendableDecimalScore, Decimal};
fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}
type Hss = BendableDecimalScore<1, 2>;
type Hhsss = BendableDecimalScore<2, 3>;
type Hhs = BendableDecimalScore<2, 1>;
type Empty = BendableDecimalScore<0, 0>;
type Soft = BendableDecimalScore<0, 1>;

#[test]
fn of() {
    assert_eq!(
        Hss::of_hard(0, d("-147")).unwrap(),
        Hss::new([d("-147")], [d("0"), d("0")])
    );
    assert_eq!(
        Hss::of_soft(0, d("-258")).unwrap(),
        Hss::new([d("0")], [d("-258"), d("0")])
    );
    assert_eq!(
        Hss::of_soft(1, d("-369")).unwrap(),
        Hss::new([d("0")], [d("0"), d("-369")])
    );
}

#[test]
fn parse_score() {
    assert_eq!(
        "[-147]hard/[-258/-369]soft".parse::<Hss>().unwrap(),
        Hss::new([d("-147")], [d("-258"), d("-369")])
    );
    assert_eq!(
        "-1structural/[-147]hard/[-258/-369]soft"
            .parse::<Hss>()
            .unwrap(),
        Hss::from_parts(-1, [d("-147")], [d("-258"), d("-369")])
    );
}

#[test]
fn to_short_string() {
    assert_eq!(
        Hss::new([d("0")], [d("0"), d("-369")]).to_short_string(),
        "[0/-369]soft"
    );
    assert_eq!(
        Hss::new([d("0")], [d("-258"), d("-369")]).to_short_string(),
        "[-258/-369]soft"
    );
    assert_eq!(
        Hss::new([d("-147")], [d("0"), d("0")]).to_short_string(),
        "[-147]hard"
    );
    assert_eq!(
        Hss::new([d("-147")], [d("-258"), d("-369")]).to_short_string(),
        "[-147]hard/[-258/-369]soft"
    );
    assert_eq!(
        Hss::from_parts(-1, [d("-147")], [d("-258"), d("-369")]).to_short_string(),
        "-1structural/[-147]hard/[-258/-369]soft"
    );
}

#[test]
fn test_to_string() {
    assert_eq!(
        (Hss::new([d("0")], [d("-258"), d("-369")])).to_string(),
        "[0]hard/[-258/-369]soft"
    );
    assert_eq!(
        (Hss::new([d("-147")], [d("-258"), d("-369")])).to_string(),
        "[-147]hard/[-258/-369]soft"
    );
    assert_eq!(
        (Hhs::new([d("-147"), d("-258")], [d("-369")])).to_string(),
        "[-147/-258]hard/[-369]soft"
    );
    assert_eq!((Empty::new([], [])).to_string(), "[]hard/[]soft");
    assert_eq!(
        (Hss::from_parts(-1, [d("-147")], [d("-258"), d("-369")])).to_string(),
        "-1structural/[-147]hard/[-258/-369]soft"
    );
}

#[test]
fn parse_score_illegal_argument() {
    assert!("-147".parse::<Hss>().is_err());
    assert!("-1structural/[-147]hard/[-258]soft".parse::<Hss>().is_err());
}

#[test]
fn get_hard_or_soft_score() {
    let score = Hss::new([d("-5")], [d("-10"), d("-200")]);
    assert_eq!(score.level(0), Some(&d("-5")));
    assert_eq!(score.level(1), Some(&d("-10")));
    assert_eq!(score.level(2), Some(&d("-200")));
}

#[test]
fn feasible_h_s_s() {
    assert!(!(Hss::new([d("-5")], [d("-300"), d("-4000")])).is_feasible());
    assert!(!(Hss::from_parts(-1, [d("0")], [d("-300"), d("-4000")])).is_feasible());
    assert!((Hss::new([d("0")], [d("-300"), d("-4000")])).is_feasible());
    assert!((Hss::new([d("2")], [d("-300"), d("-4000")])).is_feasible());
}

#[test]
fn add_h_s_s() {
    assert_eq!(
        Hss::new([d("20")], [d("-20"), d("-4000")])
            .add(Hss::new([d("-1")], [d("-300"), d("4000")])),
        Hss::new([d("19")], [d("-320"), d("0")])
    );
}

#[test]
fn subtract_h_s_s() {
    assert_eq!(
        Hss::new([d("20")], [d("-20"), d("-4000")])
            .sub(Hss::new([d("-1")], [d("-300"), d("4000")])),
        Hss::new([d("21")], [d("280"), d("-8000")])
    );
}

#[test]
fn divide_h_s_s() {
    assert_eq!(
        Hss::new([d("25")], [d("-25"), d("25")])
            .divide(&d("5.0"))
            .unwrap(),
        Hss::new([d("5")], [d("-5"), d("5")])
    );
    assert_eq!(
        Hss::new([d("21")], [d("-21"), d("21")])
            .divide(&d("5.0"))
            .unwrap(),
        Hss::new([d("4")], [d("-5"), d("4")])
    );
    assert_eq!(
        Hss::new([d("24")], [d("-24"), d("24")])
            .divide(&d("5.0"))
            .unwrap(),
        Hss::new([d("4")], [d("-5"), d("4")])
    );
}

#[test]
fn negate_h_s_s() {
    assert_eq!(
        Hss::new([d("3")], [d("-4"), d("5")]).neg(),
        Hss::new([d("-3")], [d("4"), d("-5")])
    );
    assert_eq!(
        Hss::new([d("-3")], [d("4"), d("-5")]).neg(),
        Hss::new([d("3")], [d("-4"), d("5")])
    );
}

#[test]
fn abs_h_s_s() {
    assert_eq!(
        Hss::new([d("3")], [d("4"), d("5")]).abs(),
        Hss::new([d("3")], [d("4"), d("5")])
    );
    assert_eq!(
        Hss::new([d("3")], [d("-4"), d("5")]).abs(),
        Hss::new([d("3")], [d("4"), d("5")])
    );
    assert_eq!(
        Hss::new([d("-3")], [d("4"), d("-5")]).abs(),
        Hss::new([d("3")], [d("4"), d("5")])
    );
    assert_eq!(
        Hss::new([d("-3")], [d("-4"), d("-5")]).abs(),
        Hss::new([d("3")], [d("4"), d("5")])
    );
}

#[test]
fn zero() {
    assert_eq!(Soft::ZERO, Soft::new([], [d("0")]));
    assert!(Soft::ZERO.is_zero());
    assert!(!Soft::of_soft(0, d("1")).unwrap().is_zero());
}

#[test]
fn equals_and_hash_code_h_s_s() {
    equal(&[
        Hss::new([d("-10")], [d("-200"), d("-3000")]),
        Hss::new([d("-10")], [d("-200"), d("-3000")]),
        Hss::new([d("-10.000")], [d("-200.000"), d("-3000.000")]),
    ]);
    equal(&[
        Hss::from_parts(-1, [d("-10")], [d("-200"), d("-3000")]),
        Hss::from_parts(-1, [d("-10")], [d("-200"), d("-3000")]),
    ]);
    distinct(&[
        Hss::new([d("-10")], [d("-200"), d("-3000")]),
        Hss::new([d("-30")], [d("-200"), d("-3000")]),
        Hss::new([d("-10")], [d("-400"), d("-3000")]),
        Hss::new([d("-10")], [d("-400"), d("-5000")]),
        Hss::from_parts(-1, [d("-10")], [d("-200"), d("-3000")]),
    ]);
}

#[test]
fn compare_to_h_s_s() {
    ordered(&[
        Hss::from_parts(-1, [d("-20")], [d("-2147483648"), d("-2147483648")]),
        Hss::new([d("-20")], [d("-2147483648"), d("-2147483648")]),
        Hss::new([d("-20")], [d("-2147483648"), d("-20")]),
        Hss::new([d("-20")], [d("-2147483648"), d("1")]),
        Hss::new([d("-20")], [d("-300"), d("-4000")]),
        Hss::new([d("-20")], [d("-300"), d("-300")]),
        Hss::new([d("-20")], [d("-300"), d("-20")]),
        Hss::new([d("-20")], [d("-300"), d("300")]),
        Hss::new([d("-20")], [d("-20"), d("-300")]),
        Hss::new([d("-20")], [d("-20"), d("0")]),
        Hss::new([d("-20")], [d("-20"), d("1")]),
        Hss::new([d("-1")], [d("-300"), d("-4000")]),
        Hss::new([d("-1")], [d("-300"), d("-20")]),
        Hss::new([d("-1")], [d("-20"), d("-300")]),
        Hss::new([d("1")], [d("-2147483648"), d("-20")]),
        Hss::new([d("1")], [d("-20"), d("-2147483648")]),
    ]);
}

#[test]
fn feasible_h_h_s_s_s() {
    assert!(!(Hhsss::new([d("-5"), d("0")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!(!(Hhsss::new([d("0"), d("-5")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!(!(Hhsss::new([d("1"), d("-5")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!((Hhsss::new([d("0"), d("0")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!((Hhsss::new([d("0"), d("2")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!((Hhsss::new([d("2"), d("0")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
    assert!((Hhsss::new([d("1"), d("2")], [d("-300"), d("-4000"), d("-5000")])).is_feasible());
}

#[test]
fn add_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([d("20"), d("-20")], [d("-4000"), d("0"), d("0")]).add(Hhsss::new(
            [d("-1"), d("-300")],
            [d("4000"), d("0"), d("0")]
        )),
        Hhsss::new([d("19"), d("-320")], [d("0"), d("0"), d("0")])
    );
}

#[test]
fn subtract_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([d("20"), d("-20")], [d("-4000"), d("0"), d("0")]).sub(Hhsss::new(
            [d("-1"), d("-300")],
            [d("4000"), d("0"), d("0")]
        )),
        Hhsss::new([d("21"), d("280")], [d("-8000"), d("0"), d("0")])
    );
}

#[test]
fn divide_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([d("25"), d("-25")], [d("25"), d("0"), d("0")])
            .divide(&d("5.0"))
            .unwrap(),
        Hhsss::new([d("5"), d("-5")], [d("5"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("21"), d("-21")], [d("21"), d("0"), d("0")])
            .divide(&d("5.0"))
            .unwrap(),
        Hhsss::new([d("4"), d("-5")], [d("4"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("24"), d("-24")], [d("24"), d("0"), d("0")])
            .divide(&d("5.0"))
            .unwrap(),
        Hhsss::new([d("4"), d("-5")], [d("4"), d("0"), d("0")])
    );
}

#[test]
fn multiply_clamps_negative_scale() {
    assert_eq!(
        Hhsss::new([d("1E+2"), d("5.0")], [d("0"), d("0"), d("0")])
            .multiply(&d("1.5"))
            .unwrap(),
        Hhsss::new([d("150"), d("7.5")], [d("0"), d("0"), d("0")])
    );
}

#[test]
fn divide_clamps_negative_scale() {
    assert_eq!(
        Hhsss::new([d("1E+2"), d("5.0")], [d("0"), d("0"), d("0")])
            .divide(&d("2.0"))
            .unwrap(),
        Hhsss::new([d("50"), d("2.5")], [d("0"), d("0"), d("0")])
    );
}

#[test]
fn power_clamps_negative_scale() {
    assert_eq!(
        Hhsss::new([d("1E+2"), d("5.0")], [d("0"), d("0"), d("0")])
            .power(0)
            .unwrap(),
        Hhsss::new([d("1"), d("1.0")], [d("1"), d("1"), d("1")])
    );
}

#[test]
fn negate_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([d("3"), d("-4")], [d("5"), d("0"), d("0")]).neg(),
        Hhsss::new([d("-3"), d("4")], [d("-5"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("-3"), d("4")], [d("-5"), d("0"), d("0")]).neg(),
        Hhsss::new([d("3"), d("-4")], [d("5"), d("0"), d("0")])
    );
}

#[test]
fn abs_h_h_s_s_s() {
    assert_eq!(
        Hhsss::new([d("3"), d("4")], [d("5"), d("0"), d("0")]).abs(),
        Hhsss::new([d("3"), d("4")], [d("5"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("3"), d("-4")], [d("5"), d("0"), d("0")]).abs(),
        Hhsss::new([d("3"), d("4")], [d("5"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("-3"), d("4")], [d("-5"), d("0"), d("0")]).abs(),
        Hhsss::new([d("3"), d("4")], [d("5"), d("0"), d("0")])
    );
    assert_eq!(
        Hhsss::new([d("-3"), d("-4")], [d("-5"), d("0"), d("0")]).abs(),
        Hhsss::new([d("3"), d("4")], [d("5"), d("0"), d("0")])
    );
}

#[test]
fn equals_and_hash_code_h_h_s_s_s() {
    equal(&[
        Hhsss::new([d("-10"), d("-20")], [d("-30"), d("0"), d("0")]),
        Hhsss::new([d("-10"), d("-20")], [d("-30"), d("0"), d("0")]),
    ]);
}

#[test]
fn compare_to_h_h_s_s_s() {
    ordered(&[
        Hhsss::new(
            [d("-20"), d("-2147483648")],
            [d("-2147483648"), d("0"), d("0")],
        ),
        Hhsss::new([d("-20"), d("-2147483648")], [d("-20"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-2147483648")], [d("1"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-300")], [d("-4000"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-300")], [d("-300"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-300")], [d("-20"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-300")], [d("300"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-20")], [d("-300"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-20")], [d("0"), d("0"), d("0")]),
        Hhsss::new([d("-20"), d("-20")], [d("1"), d("0"), d("0")]),
        Hhsss::new([d("-1"), d("-300")], [d("-4000"), d("0"), d("0")]),
        Hhsss::new([d("-1"), d("-300")], [d("-20"), d("0"), d("0")]),
        Hhsss::new([d("-1"), d("-20")], [d("-300"), d("0"), d("0")]),
        Hhsss::new([d("1"), d("-2147483648")], [d("-20"), d("0"), d("0")]),
        Hhsss::new([d("1"), d("-20")], [d("-2147483648"), d("0"), d("0")]),
    ]);
}

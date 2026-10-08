// Adapted from Timefold Community ScoreUtilTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Exact decimal operands replace integral/terminating-double operands; Decimal
// equality retains the upstream representation-sensitive BigDecimal assertions.
use timetuck::{BigInt, Decimal};
fn d(text: &str) -> Decimal {
    text.parse().unwrap()
}
#[test]
fn non_negative_scale() {
    for (scale, expected) in [(2, 2), (0, 0), (-2, 0)] {
        let value = Decimal::new(BigInt::ZERO, scale);
        assert_eq!(value.arithmetic_scale(), expected);
    }
}
#[test]
fn multiply_normal_scale() {
    assert_eq!(d("5.0").multiply_floor(&d("1.5")).unwrap(), d("7.5"));
}
#[test]
fn multiply_clamps_negative_scale() {
    assert_eq!(d("1E+2").multiply_floor(&d("1.5")).unwrap(), d("150"));
}
#[test]
fn divide_normal_scale() {
    assert_eq!(d("5.0").divide_floor(&d("2.0")).unwrap(), d("2.5"));
}
#[test]
fn divide_clamps_negative_scale() {
    assert_eq!(d("1E+2").divide_floor(&d("2.0")).unwrap(), d("50"));
}
#[test]
fn power_normal_scale() {
    assert_eq!(d("5.0").power_floor(2).unwrap(), d("25.0"));
}
#[test]
fn power_clamps_negative_scale() {
    assert_eq!(d("1E+2").power_floor(0).unwrap(), d("1"));
}

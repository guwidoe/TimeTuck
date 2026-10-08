//! Original public decimal arithmetic regressions; no upstream method credit.
use timetuck::{BigInt, Decimal, DecimalArithmeticError};
fn d(text: &str) -> Decimal {
    text.parse().unwrap()
}
#[test]
fn floors_negative_values_and_preserves_receiver_precision() {
    assert_eq!(d("-1.21").multiply_floor(&d("1.005")).unwrap(), d("-1.22"));
    assert_eq!(d("-1.0").divide_floor(&d("3")).unwrap(), d("-0.4"));
    assert_eq!(d("1.0").divide_floor(&d("-3")).unwrap(), d("-0.4"));
    assert_eq!(d("-1.1").power_floor(3).unwrap(), d("-1.4"));
    assert_eq!(d("-1.1").power_floor(0).unwrap(), d("1.0"));
    assert_eq!(d("3E+2").multiply_floor(&d("0.001")).unwrap(), d("0"));
    let original = d("3E+2");
    assert_eq!(original.arithmetic_scale(), 0);
    assert_eq!(original.scale(), -2);
    assert_eq!(
        d("-1.21").multiply_exact(&d("1.005")).unwrap(),
        d("-1.21605")
    );
}
#[test]
fn zero_representations_and_zero_exponents_have_explicit_results() {
    for zero in ["0", "0.00", "-0.0", "0E+3"] {
        assert_eq!(
            d("5").divide_floor(&d(zero)),
            Err(DecimalArithmeticError::DivisionByZero)
        );
        let receiver = d(zero);
        let product = receiver.multiply_floor(&d("1.23")).unwrap();
        assert_eq!(product.coefficient(), &BigInt::ZERO);
        assert_eq!(product.scale(), receiver.arithmetic_scale());
        let powered = receiver.power_floor(0).unwrap();
        assert_eq!(powered.numeric_cmp(&d("1")), std::cmp::Ordering::Equal);
        assert_eq!(powered.scale(), receiver.arithmetic_scale());
    }
}

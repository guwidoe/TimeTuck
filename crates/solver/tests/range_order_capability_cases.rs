// Adapted pinned Community value-range TESTs, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn int_sort() {
    let mut r = IntegerValueRange::new(0, 7, 1).unwrap();
    assert_eq!(r.sort(None), Err(RangeOrderError::UnsupportedSorting));
}
#[test]
fn long_sort() {
    let mut r = IntegerValueRange::new(0, 7, 1).unwrap();
    assert_eq!(r.sort(None), Err(RangeOrderError::UnsupportedSorting));
}
#[test]
fn big_integer_sort() {
    let mut r = BigIntegerValueRange::new(BigInt::from(0), BigInt::from(7)).unwrap();
    assert_eq!(r.sort(None), Err(RangeOrderError::UnsupportedSorting));
}
#[test]
fn big_decimal_sort() {
    let mut r = DecimalValueRange::new("0".parse().unwrap(), "4".parse().unwrap()).unwrap();
    assert_eq!(r.sort(None), Err(RangeOrderError::UnsupportedSorting));
}
#[test]
fn boolean_sort() {
    let mut r = BooleanValueRange;
    assert_eq!(r.sort(None), Err(RangeOrderError::UnsupportedSorting));
}

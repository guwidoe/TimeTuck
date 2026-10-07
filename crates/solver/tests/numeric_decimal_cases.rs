// Adapted from Timefold Community ScoreUtilTest, Apache-2.0.
// See compat/provenance.json. Native numeric keys replace static utilities.
use std::hash::{DefaultHasher, Hash, Hasher};
use timetuck::NumericDecimal;

fn n(text: &str) -> NumericDecimal {
    text.parse().unwrap()
}
fn hash(value: &NumericDecimal) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
#[test]
fn is_zero() {
    for text in ["0", "0.00", "-0.0"] {
        assert!(n(text).is_zero());
    }
    for text in ["1", "-1"] {
        assert!(!n(text).is_zero());
    }
}
#[test]
fn is_negative() {
    assert!(n("-1").is_negative());
    for text in ["0", "-0.0", "1"] {
        assert!(!n(text).is_negative());
    }
}
#[test]
fn equals_ignoring_scale() {
    assert_eq!(n("1.0"), n("1.00"));
    assert_ne!(n("1.0"), n("1.1"));
}
#[test]
fn hash_code_ignoring_scale() {
    assert_eq!(hash(&n("1.0")), hash(&n("1.00")));
}

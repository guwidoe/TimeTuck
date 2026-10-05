// Adapted from Community BigIntegerValueRangeTest, BigDecimalValueRangeTest and
// PlannerAssert helpers; Apache-2.0. See compat/provenance.json.
use std::{fmt::Debug, str::FromStr};
use timetuck::*;
fn big(a: &str, b: &str, step: Option<&str>) -> BigIntegerValueRange {
    match step {
        Some(s) => BigIntegerValueRange::with_step(
            a.parse().unwrap(),
            b.parse().unwrap(),
            s.parse().unwrap(),
        ),
        None => BigIntegerValueRange::new(a.parse().unwrap(), b.parse().unwrap()),
    }
    .unwrap()
}
fn dec(a: &str, b: &str, step: Option<&str>) -> DecimalValueRange {
    match step {
        Some(s) => {
            DecimalValueRange::with_step(a.parse().unwrap(), b.parse().unwrap(), s.parse().unwrap())
        }
        None => DecimalValueRange::new(a.parse().unwrap(), b.parse().unwrap()),
    }
    .unwrap()
}
fn sequence<R: ValueRange>(r: R, expected: &[&str], complete: bool)
where
    R::Value: FromStr + Debug,
    <R::Value as FromStr>::Err: Debug,
{
    let expected: Vec<R::Value> = expected.iter().map(|v| v.parse().unwrap()).collect();
    let mut it = r.iter();
    assert_eq!(
        it.by_ref().take(expected.len()).collect::<Vec<_>>(),
        expected
    );
    if complete {
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
fn random<R: ValueRange>(r: R, expected: &[&str])
where
    R::Value: FromStr + Debug,
    <R::Value as FromStr>::Err: Debug,
{
    let mut indices = [3, 0].into_iter();
    let mut it = r.random_iter_with(|_| indices.next().unwrap());
    for v in expected {
        assert_eq!(it.next(), Some(v.parse().unwrap()));
    }
    if expected.is_empty() {
        assert!(it.next().is_none());
        assert!(it.next().is_none());
    }
}
#[test]
fn big_integer_get_size() {
    for (a, b, s, n) in [
        ("0", "10", None, 10),
        ("100", "120", None, 20),
        ("-15", "25", None, 40),
        ("7", "7", None, 0),
        ("0", "10", Some("2"), 5),
        ("-1", "9", Some("2"), 5),
        ("100", "120", Some("5"), 4),
    ] {
        assert_eq!(big(a, b, s).len(), n);
    }
}
#[test]
fn big_integer_get() {
    for (a, b, s, i, v) in [
        ("0", "10", None, 3, "3"),
        ("100", "120", None, 3, "103"),
        ("-5", "25", None, 1, "-4"),
        ("-5", "25", None, 6, "1"),
        ("0", "10", Some("2"), 3, "6"),
        ("-1", "9", Some("2"), 3, "5"),
        ("100", "120", Some("5"), 3, "115"),
    ] {
        assert_eq!(big(a, b, s).get(i), Some(v.parse().unwrap()));
    }
}
#[test]
fn big_integer_contains() {
    for (a, b, s, v, yes) in [
        ("0", "10", None, "3", true),
        ("0", "10", None, "10", false),
        ("100", "120", None, "100", true),
        ("100", "120", None, "99", false),
        ("-5", "25", None, "-4", true),
        ("-5", "25", None, "-20", false),
        ("0", "10", Some("2"), "2", true),
        ("0", "10", Some("2"), "3", false),
        ("-1", "9", Some("2"), "1", true),
        ("-1", "9", Some("2"), "2", false),
        ("100", "120", Some("5"), "115", true),
        ("100", "120", Some("5"), "114", false),
    ] {
        assert_eq!(big(a, b, s).contains(&v.parse().unwrap()), yes);
    }
    assert!(!big("0", "10", None).contains_assignment(None));
}
#[test]
fn big_integer_original_iterator() {
    sequence(big("0", "4", None), &["0", "1", "2", "3"], true);
    sequence(big("100", "104", None), &["100", "101", "102", "103"], true);
    sequence(
        big("-4", "3", None),
        &["-4", "-3", "-2", "-1", "0", "1", "2"],
        true,
    );
    sequence(big("7", "7", None), &[], true);
    sequence(big("0", "10", Some("2")), &["0", "2", "4", "6", "8"], true);
    sequence(big("-1", "9", Some("2")), &["-1", "1", "3", "5", "7"], true);
    sequence(
        big("100", "120", Some("5")),
        &["100", "105", "110", "115"],
        true,
    );
}
#[test]
fn big_integer_random_iterator() {
    for (a, b, s, expected) in [
        ("0", "7", None, ["3", "0"]),
        ("100", "104", None, ["103", "100"]),
        ("-4", "3", None, ["-1", "-4"]),
        ("0", "10", Some("2"), ["6", "0"]),
        ("-1", "9", Some("2"), ["5", "-1"]),
        ("100", "120", Some("5"), ["115", "100"]),
    ] {
        random(big(a, b, s), &expected);
    }
    random(big("7", "7", None), &[]);
}
#[test]
fn decimal_get_size() {
    for (a, b, s, n) in [
        ("0", "10", None, 10),
        ("100.0", "120.0", None, 200),
        ("-15.00", "25.07", None, 4007),
        ("7.0", "7.0", None, 0),
        ("0.0", "10.0", Some("2.0"), 5),
        ("-1.0", "9.0", Some("2.0"), 5),
        ("100.0", "120.4", Some("5.1"), 4),
    ] {
        assert_eq!(dec(a, b, s).len(), n);
    }
}
#[test]
fn decimal_get() {
    for (a, b, s, i, v) in [
        ("0", "10", None, 3, "3"),
        ("100.0", "120.0", None, 3, "100.3"),
        ("-5", "25", None, 1, "-4"),
        ("-5.00", "25.00", None, 6, "-4.94"),
        ("0.0", "10.0", Some("2.0"), 3, "6.0"),
        ("-1.0", "9.0", Some("2.0"), 3, "5.0"),
        ("100.0", "120.4", Some("5.1"), 3, "115.3"),
    ] {
        assert_eq!(dec(a, b, s).get(i), Some(v.parse().unwrap()));
    }
}
#[test]
fn decimal_contains() {
    for (a, b, s, v, yes) in [
        ("0", "10", None, "3", true),
        ("0", "10", None, "10", false),
        ("100.0", "120.0", None, "100.0", true),
        ("100.0", "120.0", None, "99.9", false),
        ("-5.3", "25.2", None, "-5.2", true),
        ("-5.3", "25.2", None, "-5.4", false),
        ("0.0", "10.0", Some("2.0"), "2.0", true),
        ("0.0", "10.0", Some("2.0"), "3.0", false),
        ("-1.0", "9.0", Some("2.0"), "1.0", true),
        ("-1.0", "9.0", Some("2.0"), "2.0", false),
        ("100.0", "120.4", Some("5.1"), "115.3", true),
        ("100.0", "120.4", Some("5.1"), "115.0", false),
    ] {
        assert_eq!(dec(a, b, s).contains(&v.parse().unwrap()), yes);
    }
    assert!(!dec("0", "10", None).contains_assignment(None));
}
#[test]
fn decimal_original_iterator() {
    sequence(dec("0", "4", None), &["0", "1", "2", "3"], true);
    sequence(
        dec("100.0", "104.0", None),
        &["100.0", "100.1", "100.2"],
        false,
    );
    sequence(
        dec("-4.00", "3.00", None),
        &["-4.00", "-3.99", "-3.98"],
        false,
    );
    sequence(dec("7", "7", None), &[], true);
    sequence(
        dec("0.0", "10.0", Some("2.0")),
        &["0.0", "2.0", "4.0", "6.0", "8.0"],
        true,
    );
    sequence(
        dec("-1.0", "9.0", Some("2.0")),
        &["-1.0", "1.0", "3.0", "5.0", "7.0"],
        true,
    );
    sequence(
        dec("100.0", "120.4", Some("5.1")),
        &["100.0", "105.1", "110.2", "115.3"],
        true,
    );
}
#[test]
fn decimal_random_iterator() {
    for (a, b, s, expected) in [
        ("0", "4", None, ["3", "0"]),
        ("100.0", "104.0", None, ["100.3", "100.0"]),
        ("-4.00", "3.00", None, ["-3.97", "-4.00"]),
        ("0.0", "10.0", Some("2.0"), ["6.0", "0.0"]),
        ("-1.0", "9.0", Some("2.0"), ["5.0", "-1.0"]),
        ("100.0", "120.4", Some("5.1"), ["115.3", "100.0"]),
    ] {
        random(dec(a, b, s), &expected);
    }
    random(dec("7", "7", None), &[]);
}

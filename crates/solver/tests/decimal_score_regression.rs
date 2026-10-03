//! Original decimal API and solver regressions, no upstream method credit.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}
fn simple(s: &str) -> SimpleDecimalScore {
    s.parse().unwrap()
}
fn hash(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
#[test]
fn parsing_and_standard_text_preserve_scale_at_boundaries() {
    for (input, standard) in [
        ("+.5", "0.5"),
        ("5.", "5"),
        ("0001.20", "1.20"),
        ("-0.00", "0.00"),
        ("1e-7", "1E-7"),
        ("1e-6", "0.000001"),
        ("0e+2", "0E+2"),
        ("1e2147483648", "1E+2147483648"),
        ("1.0e2147483649", "1.0E+2147483649"),
        ("1e-2147483647", "1E-2147483647"),
    ] {
        let value = d(input);
        assert_eq!(value.to_standard_string(), standard);
        assert_eq!(d(standard), value);
    }
    for invalid in [
        "",
        ".",
        "+",
        "-",
        "1..0",
        "1e",
        "1e+",
        "1ee2",
        "1e2e3",
        " 1",
        "1 ",
        "NaN",
        "Infinity",
        "1_0",
        "١",
        "1e-2147483648",
        "1e2147483649",
        "1e9223372036854775808",
    ] {
        assert!(invalid.parse::<Decimal>().is_err(), "{invalid}");
    }
    assert_eq!(d("1.20").to_string(), "120e-2"); // Preserve the existing collector display.
}
#[test]
fn raw_zero_factory_and_explicit_structural_parse_keep_distinct_representations() {
    assert_eq!(SimpleDecimalScore::new(d("0.00")).score().scale(), 0);
    assert_eq!(simple("0.00").score().scale(), 0);
    for structural in [-1, 0, 2] {
        let s = simple(&format!("{structural}structural/0.00"));
        assert_eq!(s.score().scale(), 2);
        assert_eq!(s.structural(), structural);
        assert_eq!(
            s.to_string(),
            if structural < 0 {
                "-1structural/0.00"
            } else {
                "0.00"
            }
        );
        assert_eq!(
            s.to_short_string(),
            if structural == 0 {
                "0".into()
            } else {
                format!("{structural}structural")
            }
        );
    }
    assert_eq!(
        "*structural/1.2"
            .parse::<SimpleDecimalScore>()
            .unwrap()
            .structural(),
        i64::MIN
    );
    assert_eq!(
        HardSoftDecimalScore::new(d("0.00"), d("1.0")).to_string(),
        "0.00hard/1.0soft"
    );
    assert_eq!(
        HardSoftDecimalScore::new(d("0.00"), d("0.0")).to_string(),
        "0hard/0soft"
    );
}
#[test]
fn numeric_hash_ignores_scale_even_when_normalization_exceeds_i32() {
    for values in [
        ["1", "1.0", "10E-1"],
        ["-12", "-12.00", "-1200E-2"],
        ["0", "0E+2147483648", "0E-2147483647"],
        ["10E+2147483648", "1.0E+2147483649", "100E+2147483647"],
    ] {
        let scores = values.map(|s| SimpleDecimalScore::from_parts(0, [d(s)]));
        for a in &scores {
            for b in &scores {
                assert_eq!(a, b);
                assert_eq!(hash(a), hash(b));
                assert_eq!(a.cmp(b), std::cmp::Ordering::Equal);
            }
        }
    }
    assert_ne!(d("1"), d("1.0"));
    assert_ne!(simple("1"), simple("-1structural/1.0"));
}
#[test]
fn scale_directed_arithmetic_floors_negative_results_and_clears_structure() {
    for (a, mul, div, pow) in [
        ("1.99", "2.38", "0.99", "3.96"),
        ("-1.99", "-2.39", "-1.00", "3.96"),
    ] {
        let s = simple(&format!("-2structural/{a}"));
        assert_eq!(s.multiply(&d("1.2")).unwrap().to_string(), mul);
        assert_eq!(s.divide(&d("2")).unwrap().to_string(), div);
        assert_eq!(s.power(2).unwrap().to_string(), pow);
        assert_eq!(s.structural(), -2);
    }
    let large = simple("123456789012345678901234567890.001");
    assert_eq!(
        large.multiply(&d("2")).unwrap().to_string(),
        "246913578024691357802469135780.002"
    );
    assert_eq!(
        large.divide(&d("0")),
        Err(DecimalArithmeticError::DivisionByZero)
    );
    assert_eq!(
        simple("-1E-2147483647")
            .divide(&d("1E+2147483648"))
            .unwrap()
            .score()
            .coefficient(),
        &BigInt::from(-1)
    );
    assert_eq!(
        simple("1E-2147483647")
            .multiply(&d("1E-2147483647"))
            .unwrap(),
        SimpleDecimalScore::ZERO
    );
}
#[test]
fn negation_clamps_negative_scale_but_retains_sound_zero_representation() {
    assert_eq!((-simple("1E+2")).to_string(), "-100");
    assert_eq!(simple("-1E+2").abs().to_string(), "1E+2");
    assert_eq!((-simple("0structural/0.00")).to_string(), "0.00");
    assert_eq!((-simple("-1structural/0.00")).to_string(), "0");
    assert_eq!(
        (-HardSoftDecimalScore::from_parts(0, [d("0.00"), d("0E+2")])).to_string(),
        "0.00hard/0E+2soft"
    );
    let a = simple("-1structural/1.20");
    let b = simple("2structural/-1.200");
    assert_eq!((a + b).to_string(), "0");
}
#[test]
fn fixed_decimal_families_keep_fractional_ordering_in_exact_search() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap();
    let r = exhaustive_search(
        &p,
        |a| simple(["1.0001", "1.0003", "1.0002"][a.values()[0].unwrap()]),
        3,
    );
    assert!(r.is_optimal());
    let best = r.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(1)]);
    assert_eq!(best.score.to_string(), "1.0003");
    let r = exhaustive_search(
        &p,
        |a| {
            HardMediumSoftDecimalScore::new(
                Decimal::ZERO,
                d(["0.001", "0.002", "-0.001"][a.values()[0].unwrap()]),
                d("99999999999999999999"),
            )
        },
        3,
    );
    assert!(r.is_optimal());
    assert_eq!(r.best.unwrap().assignment.values(), &[Some(1)]);
}

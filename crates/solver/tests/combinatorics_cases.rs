// Adapted from Timefold Community MathUtilsTest at the pinned revision.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn possible_arrangements_scaled_log() {
    for owners in 2..100 {
        assert_eq!(list_arrangements_scaled_log(100, 2, 0, owners).unwrap(), 0);
    }
    assert_eq!(list_arrangements_scaled_log(100, 24, 3, 2).unwrap(), 100);
    assert_eq!(list_arrangements_scaled_log(100, 60, 3, 3).unwrap(), 100);
    // Independent exhaustive insertion replaces the upstream permutation/set helper.
    fn enumerate(value: usize, end: usize, lists: &mut [Vec<usize>]) -> u64 {
        if value == end {
            return 1;
        }
        let mut count = 0;
        for owner in 0..lists.len() {
            for index in 0..=lists[owner].len() {
                lists[owner].insert(index, value);
                count += enumerate(value + 1, end, lists);
                lists[owner].remove(index);
            }
        }
        count
    }
    for values in 3..6 {
        for owners in 1..6 {
            let log = list_arrangements_scaled_log(100000, 2, values, owners).unwrap();
            let approximate = 2f64.powf(log as f64 / 100000.0).round() as u64;
            assert_eq!(approximate, enumerate(0, values, &mut vec![vec![]; owners]));
        }
    }
}
#[test]
fn scaled_approximate_log() {
    for base in 2..100 {
        assert_eq!(scaled_log(100, base, 1).unwrap(), 0);
        assert_eq!(scaled_log(100, base, base).unwrap(), 100);
    }
    for (base, value, expected) in [
        (2, 4, 200),
        (2, 16, 400),
        (4, 16, 200),
        (3, 9, 200),
        (3, 27, 300),
        (9, 3, 50),
    ] {
        assert_eq!(scaled_log(100, base, value).unwrap(), expected);
    }
}
#[test]
fn logarithm_in_base() {
    for base in 2..100 {
        assert_eq!(log_in_base(base, 1).unwrap(), 0.0);
        assert!((log_in_base(base, base).unwrap() - 1.0).abs() <= 1e-15);
    }
    for (base, value, expected) in [
        (2, 4, 2.0),
        (2, 16, 4.0),
        (4, 16, 2.0),
        (3, 9, 2.0),
        (3, 27, 3.0),
        (9, 3, 0.5),
    ] {
        assert!((log_in_base(base, value).unwrap() - expected).abs() <= 1e-15);
    }
}
#[test]
fn factorial_values() {
    for (n, expected) in [
        1,
        1,
        2,
        6,
        24,
        120,
        720,
        5040,
        40320,
        362880,
        3628800,
        39916800,
        479001600,
        6227020800,
        87178291200,
        1307674368000,
        20922789888000,
        355687428096000,
        6402373705728000,
        121645100408832000,
        2432902008176640000,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(factorial(n as i32).unwrap(), expected);
    }
}
#[test]
fn factorial_bounds() {
    assert_eq!(
        factorial(-1).unwrap_err().to_string(),
        "Factorial of -1 is undefined."
    );
    assert_eq!(
        factorial(21).unwrap_err().to_string(),
        "Factorial of 21 is too large to fit in a long."
    );
}
#[test]
fn binomial_values() {
    for (n, k, expected) in [
        (0, 0, 1),
        (1, 0, 1),
        (1, 1, 1),
        (5, 0, 1),
        (5, 1, 5),
        (5, 2, 10),
        (5, 3, 10),
        (5, 4, 5),
        (5, 5, 1),
        (10, 0, 1),
        (10, 1, 10),
        (10, 2, 45),
        (10, 3, 120),
        (10, 4, 210),
        (10, 5, 252),
        (10, 6, 210),
        (10, 7, 120),
        (10, 8, 45),
        (10, 9, 10),
        (10, 10, 1),
    ] {
        assert_eq!(binomial_coefficient(n, k).unwrap(), expected);
    }
}

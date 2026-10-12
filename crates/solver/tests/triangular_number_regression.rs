// Original complete safe input domain, inverse bracket and negative controls.
use timetuck::*;
#[test]
fn complete_safe_domain_is_monotonic_and_roundtrips_with_exact_wide_reference() {
    for n in 0..=HIGHEST_SAFE_TRIANGULAR_N {
        let exact = i64::from(n) * (i64::from(n) + 1) / 2;
        let value = nth_triangle(n).unwrap();
        assert_eq!(i64::from(value), exact);
        assert_eq!(triangular_root(value).unwrap(), f64::from(n));
        if n > 0 {
            assert_eq!(value - nth_triangle(n - 1).unwrap(), n);
        }
        if n > 1 {
            let below = triangular_root(value - 1).unwrap();
            assert!(below > f64::from(n - 1));
            assert!(below < f64::from(n));
        }
    }
}
#[test]
fn negative_arguments_and_all_overflow_boundaries_reject() {
    for n in [-1, i32::MIN] {
        assert_eq!(nth_triangle(n), Err(TriangularError::NegativeArgument));
        assert_eq!(triangular_root(n), Err(TriangularError::NegativeArgument));
    }
    for n in [HIGHEST_SAFE_TRIANGULAR_N + 1, i32::MAX] {
        assert_eq!(nth_triangle(n), Err(TriangularError::IntegerOverflow));
    }
    let root = triangular_root(i32::MAX).unwrap();
    assert!(root > 65535.0 && root < 65536.0);
}

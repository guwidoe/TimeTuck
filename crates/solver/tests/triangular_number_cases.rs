// Complete TriangularNumbersTest adaptation, Apache-2.0; see provenance/notices.
use timetuck::*;
const CASES: [(i32, i32); 8] = [
    (0, 0),
    (1, 1),
    (2, 3),
    (3, 6),
    (4, 10),
    (5, 15),
    (23169, 268_412_865),
    (46_340, 1_073_720_970),
];
#[test]
fn overflow() {
    let error = nth_triangle(HIGHEST_SAFE_TRIANGULAR_N + 1).unwrap_err();
    assert_eq!(error, TriangularError::IntegerOverflow);
    assert_eq!(error.to_string(), "integer overflow");
}
#[test]
fn nth_triangle_case() {
    for (n, expected) in CASES {
        assert_eq!(nth_triangle(n).unwrap(), expected);
    }
}
#[test]
fn triangular_root_case() {
    for (n, value) in CASES {
        assert_eq!(triangular_root(value).unwrap(), f64::from(n));
    }
}
#[test]
fn non_triangular_roots() {
    for (value, lower) in [
        (2, 1),
        (4, 2),
        (5, 2),
        (7, 3),
        (8, 3),
        (9, 3),
        (268_435_456, 23169),
    ] {
        let root = triangular_root(value).unwrap();
        assert!(root > f64::from(lower));
        assert!(root < f64::from(lower + 1));
    }
}

// Original custom temporal endpoint, signed bounds, sorting and law guards.
use std::{cell::Cell, cmp::Ordering};
use timetuck::*;
#[derive(Clone, Copy, Debug)]
struct Point {
    value: i64,
    supports: bool,
}
impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Point {}
impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Point {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}
impl TemporalValue for Point {
    fn supports_unit(self, unit: TemporalUnit) -> bool {
        self.supports && unit == TemporalUnit::Days
    }
    fn offset(self, amount: i128, _: TemporalUnit) -> Option<Self> {
        Some(Self {
            value: i64::try_from(i128::from(self.value).checked_add(amount)?).ok()?,
            ..self
        })
    }
    fn distance(self, end: Self, _: TemporalUnit) -> Option<i128> {
        Some(i128::from(end.value) - i128::from(self.value))
    }
}
fn point(value: i64, supports: bool) -> Point {
    Point { value, supports }
}
#[test]
fn custom_temporal_progressions_anchor_offsets_and_validate_both_endpoints() {
    for (a, b) in [(false, true), (true, false), (false, false)] {
        assert_eq!(
            TemporalValueRange::new(point(10, a), point(22, b), 3, TemporalUnit::Days).unwrap_err(),
            TemporalRangeError::UnsupportedUnit
        );
    }
    let range =
        TemporalValueRange::new(point(10, true), point(22, true), 3, TemporalUnit::Days).unwrap();
    assert_eq!(range.len(), 4);
    assert_eq!(
        range.iter().map(|p| p.value).collect::<Vec<_>>(),
        vec![10, 13, 16, 19]
    );
    assert!(range.contains(&point(19, true)));
    assert!(!range.contains(&point(20, true)));
    assert!(!range.contains(&point(22, true)));
}
#[test]
fn checked_signed_bounds_and_sort_rejection_preserve_values_without_comparator_calls() {
    let mut range =
        TemporalValueRange::new(point(10, true), point(22, true), 3, TemporalUnit::Days).unwrap();
    for index in [-1, 4, i128::MAX] {
        assert_eq!(
            range.get_signed(index),
            Err(TemporalRangeError::IndexOutOfBounds { index, size: 4 })
        );
    }
    assert_eq!(range.get_signed(0).unwrap().value, 10);
    assert_eq!(range.get_signed(3).unwrap().value, 19);
    let calls = Cell::new(0);
    let compare = |a: &Point, b: &Point| {
        calls.set(calls.get() + 1);
        b.cmp(a)
    };
    assert_eq!(
        range.sort(Some(&compare)),
        Err(TemporalRangeError::UnsupportedSorting)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        range.iter().map(|p| p.value).collect::<Vec<_>>(),
        vec![10, 13, 16, 19]
    );
}
#[test]
fn inconsistent_negative_distance_is_rejected_before_count_conversion() {
    #[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
    struct Broken(u8);
    impl TemporalValue for Broken {
        fn supports_unit(self, _: TemporalUnit) -> bool {
            true
        }
        fn offset(self, _: i128, _: TemporalUnit) -> Option<Self> {
            Some(Self(1))
        }
        fn distance(self, _: Self, _: TemporalUnit) -> Option<i128> {
            Some(-1)
        }
    }
    assert!(matches!(
        TemporalValueRange::new(Broken(0), Broken(1), 1, TemporalUnit::Days),
        Err(TemporalRangeError::UnalignedEnd)
    ));
}

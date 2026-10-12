// Adapted remaining TemporalValueRangeTest methods, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cmp::Ordering;
use timetuck::*;
#[derive(Clone, Copy, Debug)]
struct TemporalMock {
    supports: bool,
}
// Upstream mock compareTo has its default zero result.
impl PartialEq for TemporalMock {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for TemporalMock {}
impl PartialOrd for TemporalMock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TemporalMock {
    fn cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl TemporalValue for TemporalMock {
    fn supports_unit(self, _: TemporalUnit) -> bool {
        self.supports
    }
    fn offset(self, _: i128, _: TemporalUnit) -> Option<Self> {
        panic!("unsupported endpoint must fail before arithmetic")
    }
    fn distance(self, _: Self, _: TemporalUnit) -> Option<i128> {
        panic!("unsupported endpoint must fail before arithmetic")
    }
}
#[test]
fn increment_unit_type_not_supported_by_from() {
    // Preserve actual stubbing: the source overwrites from=false with from=true;
    // the unstubbed to endpoint remains false, despite the original test name.
    assert_eq!(
        TemporalValueRange::new(
            TemporalMock { supports: true },
            TemporalMock { supports: false },
            1,
            TemporalUnit::Years
        )
        .unwrap_err(),
        TemporalRangeError::UnsupportedUnit
    );
}
#[test]
fn increment_unit_type_not_supported_by_to() {
    // Source overwrites from=true with from=false; to remains unstubbed false.
    assert_eq!(
        TemporalValueRange::new(
            TemporalMock { supports: false },
            TemporalMock { supports: false },
            1,
            TemporalUnit::Years
        )
        .unwrap_err(),
        TemporalRangeError::UnsupportedUnit
    );
}
#[test]
fn get_index_negative() {
    let range = TemporalValueRange::new(
        CalendarYear::new(0).unwrap(),
        CalendarYear::new(1).unwrap(),
        1,
        TemporalUnit::Years,
    )
    .unwrap();
    assert!(matches!(
        range.get_signed(-1),
        Err(TemporalRangeError::IndexOutOfBounds { index: -1, size: 1 })
    ));
}
#[test]
fn sort() {
    let mut range = TemporalValueRange::new(
        CalendarYear::new(0).unwrap(),
        CalendarYear::new(1).unwrap(),
        1,
        TemporalUnit::Years,
    )
    .unwrap();
    assert_eq!(
        range.sort(None),
        Err(TemporalRangeError::UnsupportedSorting)
    );
}

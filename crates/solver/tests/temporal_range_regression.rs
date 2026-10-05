use std::sync::Arc;
use timetuck::*;
fn date(y: i32, m: u8, d: u8) -> CivilDate {
    CivilDate::new(y, m, d).unwrap()
}
#[test]
fn calendar_roundtrips_negative_and_positive_complete_leap_cycles() {
    // Independent civil enumeration, including century exceptions and year zero.
    for start in [-800, -200, 1800] {
        let mut number = date(start, 1, 1).day_number();
        for year in start..start + 400 {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            for (m, days) in [
                31,
                28 + i32::from(leap),
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ]
            .into_iter()
            .enumerate()
            {
                for day in 1..=days {
                    let value = date(year, m as u8 + 1, day as u8);
                    assert_eq!(value.day_number(), number);
                    assert_eq!(CivilDate::from_day_number(number), Some(value));
                    number += 1;
                }
            }
        }
        assert_eq!(number - date(start, 1, 1).day_number(), 146097);
    }
    for value in [
        CivilDate::MIN,
        CivilDate::MAX,
        date(0, 1, 1),
        date(-400, 2, 29),
    ] {
        assert_eq!(CivilDate::from_day_number(value.day_number()), Some(value));
    }
    assert_eq!(
        CivilDate::from_day_number(CivilDate::MIN.day_number() - 1),
        None
    );
    assert_eq!(
        CivilDate::from_day_number(CivilDate::MAX.day_number() + 1),
        None
    );
}
#[test]
fn month_and_year_strides_anchor_without_leap_day_drift() {
    let start = date(2000, 2, 29);
    let range = TemporalValueRange::new(start, date(2008, 2, 29), 1, TemporalUnit::Years).unwrap();
    let expected = [
        date(2000, 2, 29),
        date(2001, 2, 28),
        date(2002, 2, 28),
        date(2003, 2, 28),
        date(2004, 2, 29),
        date(2005, 2, 28),
        date(2006, 2, 28),
        date(2007, 2, 28),
    ];
    assert_eq!(range.iter().collect::<Vec<_>>(), expected);
    for value in expected {
        assert!(range.contains(&value));
    }
    assert!(!range.contains(&date(2004, 2, 28)));
    let range = TemporalValueRange::new(date(-1, 10, 31), date(0, 10, 31), 3, TemporalUnit::Months)
        .unwrap();
    assert_eq!(
        range.iter().rev().collect::<Vec<_>>(),
        [
            date(0, 7, 31),
            date(0, 4, 30),
            date(0, 1, 31),
            date(-1, 10, 31)
        ]
    );
    assert!(!range.contains(&date(0, 2, 29)));
    for unit in [
        TemporalUnit::Decades,
        TemporalUnit::Centuries,
        TemporalUnit::Millennia,
    ] {
        let end = start.offset(2, unit).unwrap();
        let range = TemporalValueRange::new(start, end, 1, unit).unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range.get(1), start.offset(1, unit));
    }
}
#[test]
fn nanosecond_ranges_span_midnight_and_huge_calendars_without_materializing() {
    let start = CivilDateTime::new(
        date(-1, 12, 31),
        CivilTime::new(23, 59, 59, 999_999_998).unwrap(),
    );
    let end = CivilDateTime::new(date(0, 1, 1), CivilTime::new(0, 0, 0, 2).unwrap());
    let range = TemporalValueRange::new(start, end, 1, TemporalUnit::Nanoseconds).unwrap();
    assert_eq!(range.len(), 4);
    assert_eq!(
        range.get(2),
        Some(CivilDateTime::new(date(0, 1, 1), CivilTime::MIN))
    );
    for value in range.iter() {
        assert!(range.contains(&value));
    }
    let huge = TemporalValueRange::new(
        CivilDateTime::MIN,
        CivilDateTime::MAX,
        1,
        TemporalUnit::Nanoseconds,
    )
    .unwrap();
    assert!(huge.len() > u64::MAX as u128);
    assert_eq!(
        huge.get(huge.len() - 1),
        Some(CivilDateTime::new(
            CivilDate::MAX,
            CivilTime::new(23, 59, 59, 999_999_998).unwrap()
        ))
    );
    assert_eq!(huge.get(u128::MAX), None);
    assert!(huge.materialize(10).is_err());
    for value in huge.random_iter(7).take(100) {
        assert!(huge.contains(&value));
    }
}
#[test]
fn reject_invalid_dates_units_steps_and_unreachable_endpoints() {
    for (y, m, d) in [
        (1900, 2, 29),
        (2000, 0, 1),
        (2000, 13, 1),
        (2024, 4, 31),
        (2024, 1, 0),
        (1_000_000_000, 1, 1),
    ] {
        assert!(CivilDate::new(y, m, d).is_err());
    }
    for (h, m, s, n) in [
        (24, 0, 0, 0),
        (0, 60, 0, 0),
        (0, 0, 60, 0),
        (0, 0, 0, 1_000_000_000),
    ] {
        assert!(CivilTime::new(h, m, s, n).is_err());
    }
    assert!(CalendarYear::new(i32::MIN).is_err());
    assert!(CalendarYear::new(i32::MAX).is_err());
    let d = date(2024, 1, 31);
    assert_eq!(
        TemporalValueRange::new(d, d, 1, TemporalUnit::Hours),
        Err(TemporalRangeError::UnsupportedUnit)
    );
    assert_eq!(
        TemporalValueRange::new(CivilTime::MIN, CivilTime::MAX, 1, TemporalUnit::Days),
        Err(TemporalRangeError::UnsupportedUnit)
    );
    assert_eq!(
        TemporalValueRange::new(
            CalendarYear::MIN,
            CalendarYear::MAX,
            1,
            TemporalUnit::Months
        ),
        Err(TemporalRangeError::UnsupportedUnit)
    );
    assert_eq!(
        TemporalValueRange::new(d, d, -1, TemporalUnit::Months),
        Err(TemporalRangeError::NonPositiveStep)
    );
    assert_eq!(
        TemporalValueRange::new(d, date(2024, 3, 30), 1, TemporalUnit::Months),
        Err(TemporalRangeError::UnalignedEnd)
    );
    assert_eq!(
        TemporalValueRange::new(d, date(2024, 2, 7), 2, TemporalUnit::Days),
        Err(TemporalRangeError::UnalignedEnd)
    );
    let a = CivilDateTime::new(d, CivilTime::MIN);
    let b = CivilDateTime::new(date(2024, 2, 29), CivilTime::new(0, 0, 0, 1).unwrap());
    assert_eq!(
        TemporalValueRange::new(a, b, 1, TemporalUnit::Months),
        Err(TemporalRangeError::UnalignedEnd)
    );
    assert_eq!(d.offset(i128::MAX, TemporalUnit::Millennia), None);
    assert_eq!(
        CivilDateTime::MIN.offset(i128::MAX, TemporalUnit::Weeks),
        None
    );
    assert_eq!(CivilTime::MIN.offset(i128::MAX, TemporalUnit::Hours), None);
    assert_eq!(
        CalendarYear::MIN.offset(i128::MAX, TemporalUnit::Millennia),
        None
    );
}
#[test]
fn clock_units_preserve_precision_and_never_wrap() {
    let start = CivilTime::new(1, 2, 3, 4005).unwrap();
    assert_eq!(
        (
            start.hour(),
            start.minute(),
            start.second(),
            start.nanosecond()
        ),
        (1, 2, 3, 4005)
    );
    for unit in [
        TemporalUnit::Nanoseconds,
        TemporalUnit::Microseconds,
        TemporalUnit::Milliseconds,
        TemporalUnit::Seconds,
        TemporalUnit::Minutes,
        TemporalUnit::Hours,
        TemporalUnit::HalfDays,
    ] {
        let end = start.offset(1, unit).unwrap();
        let range = TemporalValueRange::new(start, end, 1, unit).unwrap();
        assert_eq!(range.iter().collect::<Vec<_>>(), [start]);
        assert!(!range.contains(&end));
    }
    assert_eq!(CivilTime::MAX.offset(1, TemporalUnit::Nanoseconds), None);
    assert_eq!(CivilTime::MIN.offset(-1, TemporalUnit::Nanoseconds), None);
    let range = TemporalValueRange::new(
        CivilTime::MIN,
        CivilTime::new(1, 0, 0, 0).unwrap(),
        15,
        TemporalUnit::Minutes,
    )
    .unwrap();
    assert_eq!(range.len(), 4);
    assert!(!range.contains(&CivilTime::new(0, 14, 59, 999_999_999).unwrap()));
    assert_eq!(
        range.iter().nth(3),
        Some(CivilTime::new(0, 45, 0, 0).unwrap())
    );
}
#[test]
fn date_ranges_compose_and_intersect_with_optional_destinations() {
    let a: SharedValueRange<'_, CivilDate> = Arc::new(
        TemporalValueRange::new(
            date(2024, 1, 31),
            date(2024, 5, 31),
            1,
            TemporalUnit::Months,
        )
        .unwrap(),
    );
    let b: SharedValueRange<'_, CivilDate> = Arc::new(
        TemporalValueRange::new(date(2024, 2, 1), date(2024, 3, 1), 1, TemporalUnit::Days).unwrap(),
    );
    let common = CommonValueRanges::new([a, b]).unwrap();
    assert_eq!(
        common.pick_exact(None, 0, 4).unwrap(),
        Some(date(2024, 2, 29))
    );
    let optional = OptionalValueRange::new(
        TemporalValueRange::new(date(2024, 2, 28), date(2024, 3, 1), 1, TemporalUnit::Days)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        optional.iter().collect::<Vec<_>>(),
        [None, Some(date(2024, 2, 28)), Some(date(2024, 2, 29))]
    );
}
#[test]
fn solve_calendar_assignments_with_date_constraints() {
    let range = TemporalValueRange::new(date(2024, 2, 28), date(2024, 3, 2), 1, TemporalUnit::Days)
        .unwrap();
    let p = FiniteProblem::new(vec![
        variable_from_range(&range, 3).unwrap(),
        variable_from_range(&range, 3).unwrap(),
    ])
    .unwrap();
    // Appointments must differ; prefer the earliest possible pair of dates.
    let result = exhaustive_search(
        &p,
        |a| {
            let x = a.values()[0].unwrap();
            let y = a.values()[1].unwrap();
            HardSoftScore::new(
                if x == y { -1 } else { 0 },
                -((x.day_number() - date(2024, 2, 28).day_number())
                    + (y.day_number() - date(2024, 2, 28).day_number())),
            )
        },
        100,
    );
    assert_eq!(result.evaluations, 9);
    let best = result.best.unwrap();
    assert_eq!(best.score, HardSoftScore::new(0, -1));
    assert_ne!(best.assignment.values()[0], best.assignment.values()[1]);
}

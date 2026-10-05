// Adapted from Timefold Community TemporalValueRangeTest and PlannerAssert;
// Apache-2.0. See compat/provenance.json.
use timetuck::*;
fn date(y: i32, m: u8, d: u8) -> CivilDate {
    CivilDate::new(y, m, d).unwrap()
}
fn dt(y: i32, m: u8, d: u8, h: u8, min: u8, s: u8) -> CivilDateTime {
    CivilDateTime::new(date(y, m, d), CivilTime::new(h, min, s, 0).unwrap())
}
fn r<T: TemporalValue>(a: T, b: T, step: i64, unit: TemporalUnit) -> TemporalValueRange<T> {
    TemporalValueRange::new(a, b, step, unit).unwrap()
}
fn sequence<T: TemporalValue + std::fmt::Debug>(range: TemporalValueRange<T>, expected: &[T]) {
    let mut it = range.iter();
    for value in expected {
        assert_eq!(it.next(), Some(*value));
    }
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
}
fn random<T: TemporalValue + std::fmt::Debug>(
    range: TemporalValueRange<T>,
    ranks: &[u128],
    expected: &[T],
) {
    let mut ranks = ranks.iter();
    let mut it = range.random_iter_with(|_| *ranks.next().unwrap());
    for value in expected {
        assert_eq!(it.next(), Some(*value));
    }
}

#[test]
fn get_size_for_local_date() {
    let from = date(2016, 7, 1);
    let to = date(2016, 7, 8);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 7);
    assert_eq!(r(from, to, 7, TemporalUnit::Days).len(), 1);
    assert_eq!(r(from, to, 1, TemporalUnit::Weeks).len(), 1);
    let from = date(2016, 7, 7);
    let to = date(2016, 7, 17);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 10);
    assert_eq!(r(from, to, 2, TemporalUnit::Days).len(), 5);
    assert_eq!(r(from, to, 5, TemporalUnit::Days).len(), 2);
    let from = date(2016, 7, 7);
    let to = date(2016, 7, 7);
    assert_eq!(r(from, to, 1, TemporalUnit::Months).len(), 0);
    let from = date(2017, 1, 31);
    let to = date(2017, 2, 28);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 28);
    assert_eq!(r(from, to, 2, TemporalUnit::Days).len(), 14);
    assert_eq!(r(from, to, 1, TemporalUnit::Months).len(), 1);
    let from = date(2016, 1, 1);
    let to = date(2016, 7, 17);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 198);
    assert_eq!(r(from, to, 2, TemporalUnit::Days).len(), 99);
    let from = date(2016, 1, 1);
    let to = date(2016, 7, 1);
    assert_eq!(r(from, to, 1, TemporalUnit::Months).len(), 6);
    let from = date(2015, 12, 20);
    let to = date(2016, 1, 21);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 32);
    assert_eq!(r(from, to, 2, TemporalUnit::Days).len(), 16);
    let from = date(2015, 12, 20);
    let to = date(2016, 1, 20);
    assert_eq!(r(from, to, 1, TemporalUnit::Months).len(), 1);
    let from = date(1960, 12, 24);
    let to = date(2050, 7, 7);
    assert_eq!(r(from, to, 1, TemporalUnit::Days).len(), 32702);
    assert_eq!(r(from, to, 2, TemporalUnit::Days).len(), 16351);
    let from = date(1960, 12, 24);
    let to = date(2050, 6, 24);
    assert_eq!(r(from, to, 1, TemporalUnit::Months).len(), 1074);
    let from = date(1960, 12, 24);
    let to = date(2049, 12, 24);
    assert_eq!(r(from, to, 1, TemporalUnit::Years).len(), 89);
}

#[test]
fn get_size_for_local_date_time() {
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 7, 7, 7, 7, 7);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Months).len(), 0);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Days).len(), 0);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Hours).len(), 0);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Minutes).len(), 0);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Seconds).len(), 0);
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 7, 7, 7, 7, 8);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Seconds).len(), 1);
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 12, 7, 7, 7, 7);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Months).len(), 5);
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 12, 12, 7, 7, 7);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Days).len(), 158);
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 12, 12, 12, 7, 7);
    assert_eq!(r(from_time, to_time, 1, TemporalUnit::Hours).len(), 3797);
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 12, 12, 12, 12, 7);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Minutes).len(),
        227825
    );
    let from_time = dt(2016, 7, 7, 7, 7, 7);
    let to_time = dt(2016, 12, 12, 12, 12, 12);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Seconds).len(),
        13669505
    );
}

#[test]
fn get_for_local_date() {
    let from = date(2016, 7, 1);
    let to = date(2016, 7, 8);
    assert_eq!(
        r(from, to, 1, TemporalUnit::Days).get(0),
        Some(date(2016, 7, 1))
    );
    assert_eq!(
        r(from, to, 1, TemporalUnit::Days).get(1),
        Some(date(2016, 7, 2))
    );
    assert_eq!(
        r(from, to, 1, TemporalUnit::Days).get(6),
        Some(date(2016, 7, 7))
    );
    let from = date(2016, 1, 1);
    let to = date(2016, 7, 1);
    assert_eq!(
        r(from, to, 1, TemporalUnit::Months).get(0),
        Some(date(2016, 1, 1))
    );
    assert_eq!(
        r(from, to, 1, TemporalUnit::Months).get(5),
        Some(date(2016, 6, 1))
    );
    let from = date(1992, 1, 1);
    let to = date(2016, 1, 1);
    assert_eq!(
        r(from, to, 1, TemporalUnit::Years).get(0),
        Some(date(1992, 1, 1))
    );
    assert_eq!(
        r(from, to, 1, TemporalUnit::Years).get(23),
        Some(date(2015, 1, 1))
    );
}

#[test]
fn get_for_local_date_time() {
    let from_time = dt(2016, 1, 1, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 7, 7);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Seconds).get(0),
        Some(dt(2016, 1, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Seconds).get(1),
        Some(dt(2016, 1, 1, 1, 1, 2))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Seconds).get(16265165),
        Some(dt(2016, 7, 7, 7, 7, 6))
    );
    let from_time = dt(2016, 1, 1, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 7, 1);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Minutes).get(0),
        Some(dt(2016, 1, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Minutes).get(1),
        Some(dt(2016, 1, 1, 1, 2, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Minutes).get(271085),
        Some(dt(2016, 7, 7, 7, 6, 1))
    );
    let from_time = dt(2016, 1, 1, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 1, 1);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Hours).get(0),
        Some(dt(2016, 1, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Hours).get(1),
        Some(dt(2016, 1, 1, 2, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Hours).get(4517),
        Some(dt(2016, 7, 7, 6, 1, 1))
    );
    let from_time = dt(2016, 1, 1, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 1, 1, 1);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Days).get(0),
        Some(dt(2016, 1, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Days).get(1),
        Some(dt(2016, 1, 2, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Days).get(187),
        Some(dt(2016, 7, 6, 1, 1, 1))
    );
    let from_time = dt(2016, 1, 1, 1, 1, 1);
    let to_time = dt(2016, 7, 1, 1, 1, 1);
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Months).get(0),
        Some(dt(2016, 1, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Months).get(1),
        Some(dt(2016, 2, 1, 1, 1, 1))
    );
    assert_eq!(
        r(from_time, to_time, 1, TemporalUnit::Months).get(5),
        Some(dt(2016, 6, 1, 1, 1, 1))
    );
}

#[test]
fn contains_for_local_date() {
    let from = date(2016, 7, 1);
    let to = date(2016, 9, 8);
    assert!(!r(from, to, 1, TemporalUnit::Days).contains(&date(2016, 6, 30)));
    assert!(!r(from, to, 1, TemporalUnit::Days).contains(&date(2015, 7, 1)));
    assert!(r(from, to, 1, TemporalUnit::Days).contains(&date(2016, 7, 1)));
    assert!(r(from, to, 1, TemporalUnit::Days).contains(&date(2016, 7, 3)));
    assert!(r(from, to, 1, TemporalUnit::Days).contains(&date(2016, 9, 7)));
    assert!(!r(from, to, 1, TemporalUnit::Days).contains(&date(2016, 9, 8)));
    let from = date(2016, 7, 1);
    let to = date(2016, 9, 1);
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2016, 6, 1)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2016, 7, 1)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2016, 8, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2016, 9, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2016, 7, 7)));
    let from = date(2017, 1, 31);
    let to = date(2017, 2, 28);
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 30)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 31)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 27)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 28)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 1)));
    let from = date(2017, 1, 31);
    let to = date(2017, 3, 31);
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 30)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 31)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 27)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 28)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 30)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 31)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 4, 1)));
    let from = date(2017, 1, 31);
    let to = date(2017, 4, 30);
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 30)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 1, 31)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 27)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 2, 28)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 1)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 30)));
    assert!(r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 3, 31)));
    assert!(!r(from, to, 1, TemporalUnit::Months).contains(&date(2017, 4, 1)));
}

#[test]
fn contains_for_local_date_time() {
    let from_time = dt(2016, 7, 7, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 1, 1);
    assert!(!r(from_time, to_time, 1, TemporalUnit::Hours).contains(&dt(2016, 7, 6, 23, 59, 59)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Hours).contains(&dt(2016, 7, 7, 1, 1, 1)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Hours).contains(&dt(2016, 7, 7, 6, 1, 1)));
    assert!(!r(from_time, to_time, 1, TemporalUnit::Hours).contains(&dt(2016, 7, 7, 7, 7, 7)));
    let from_time = dt(2016, 7, 7, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 7, 1);
    assert!(!r(from_time, to_time, 1, TemporalUnit::Minutes).contains(&dt(2016, 7, 7, 0, 59, 59)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Minutes).contains(&dt(2016, 7, 7, 1, 1, 1)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Minutes).contains(&dt(2016, 7, 7, 7, 6, 1)));
    assert!(!r(from_time, to_time, 1, TemporalUnit::Minutes).contains(&dt(2016, 7, 7, 7, 7, 1)));
    let from_time = dt(2016, 7, 7, 1, 1, 1);
    let to_time = dt(2016, 7, 7, 7, 7, 7);
    assert!(!r(from_time, to_time, 1, TemporalUnit::Seconds).contains(&dt(2016, 7, 7, 1, 0, 59)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Seconds).contains(&dt(2016, 7, 7, 1, 1, 1)));
    assert!(r(from_time, to_time, 1, TemporalUnit::Seconds).contains(&dt(2016, 7, 7, 7, 7, 6)));
    assert!(!r(from_time, to_time, 1, TemporalUnit::Seconds).contains(&dt(2016, 7, 7, 7, 7, 7)));
}

#[test]
fn create_original_iterator_for_local_date() {
    let from = date(2016, 7, 1);
    let to = date(2016, 7, 10);
    sequence(
        r(from, to, 1, TemporalUnit::Days),
        &[
            date(2016, 7, 1),
            date(2016, 7, 2),
            date(2016, 7, 3),
            date(2016, 7, 4),
            date(2016, 7, 5),
            date(2016, 7, 6),
            date(2016, 7, 7),
            date(2016, 7, 8),
            date(2016, 7, 9),
        ],
    );
    sequence(
        r(from, to, 3, TemporalUnit::Days),
        &[date(2016, 7, 1), date(2016, 7, 4), date(2016, 7, 7)],
    );
    let from = date(2016, 9, 3);
    let to = date(2017, 3, 3);
    sequence(
        r(from, to, 1, TemporalUnit::Months),
        &[
            date(2016, 9, 3),
            date(2016, 10, 3),
            date(2016, 11, 3),
            date(2016, 12, 3),
            date(2017, 1, 3),
            date(2017, 2, 3),
        ],
    );
    let from = date(1999, 9, 3);
    let to = date(2003, 9, 3);
    sequence(
        r(from, to, 1, TemporalUnit::Years),
        &[
            date(1999, 9, 3),
            date(2000, 9, 3),
            date(2001, 9, 3),
            date(2002, 9, 3),
        ],
    );
}

#[test]
fn create_original_iterator_for_local_date_time() {
    let from_time = dt(2016, 7, 1, 4, 5, 12);
    let to_time = dt(2016, 7, 3, 4, 5, 12);
    sequence(
        r(from_time, to_time, 1, TemporalUnit::Days),
        &[dt(2016, 7, 1, 4, 5, 12), dt(2016, 7, 2, 4, 5, 12)],
    );
    let from_time = dt(2016, 7, 1, 23, 5, 12);
    let to_time = dt(2016, 7, 2, 5, 5, 12);
    sequence(
        r(from_time, to_time, 1, TemporalUnit::Hours),
        &[
            dt(2016, 7, 1, 23, 5, 12),
            dt(2016, 7, 2, 0, 5, 12),
            dt(2016, 7, 2, 1, 5, 12),
            dt(2016, 7, 2, 2, 5, 12),
            dt(2016, 7, 2, 3, 5, 12),
            dt(2016, 7, 2, 4, 5, 12),
        ],
    );
}

#[test]
fn create_random_iterator_for_local_date() {
    let from = date(2016, 7, 1);
    let to = date(2016, 7, 11);
    random(
        r(from, to, 1, TemporalUnit::Days),
        &[3, 0],
        &[date(2016, 7, 4)],
    );
    random(
        r(from, to, 1, TemporalUnit::Days),
        &[0],
        &[date(2016, 7, 1)],
    );
    random(
        r(from, to, 5, TemporalUnit::Days),
        &[1, 0],
        &[date(2016, 7, 6)],
    );
}

#[test]
fn create_random_iterator_for_local_date_time() {
    let from_time = dt(2016, 7, 1, 4, 5, 12);
    let to_time = dt(2016, 7, 3, 12, 15, 12);
    random(
        r(from_time, to_time, 10, TemporalUnit::Minutes),
        &[3, 0],
        &[dt(2016, 7, 1, 4, 35, 12)],
    );
}
#[test]
fn empty_random_iterator() {
    let range = r(
        CalendarYear::new(0).unwrap(),
        CalendarYear::new(0).unwrap(),
        1,
        TemporalUnit::Years,
    );
    assert_eq!(range.random_iter(0).next(), None);
}
#[test]
fn get_and_create_original_iterator_in_sync_for_local_date() {
    let range = r(
        date(2016, 1, 31),
        date(2016, 7, 31),
        1,
        TemporalUnit::Months,
    );
    let expected = [
        date(2016, 1, 31),
        date(2016, 2, 29),
        date(2016, 3, 31),
        date(2016, 4, 30),
        date(2016, 5, 31),
        date(2016, 6, 30),
    ];
    assert_eq!(range.len(), 6);
    for (i, value) in expected.iter().enumerate() {
        assert_eq!(range.get(i as u128), Some(*value));
    }
    sequence(range, &expected);
}
#[test]
fn full_local_date_range() {
    let range = r(CivilDate::MIN, CivilDate::MAX, 4093, TemporalUnit::Days);
    assert_eq!(
        range.len() * 4093,
        (CivilDate::MAX.day_number() - CivilDate::MIN.day_number()) as u128
    );
    assert!(range.contains(&CivilDate::MIN));
    assert!(!range.contains(&CivilDate::MAX));
}
#[test]
fn full_local_date_time_range() {
    let end = CivilDateTime::new(CivilDate::MAX, CivilTime::MIN);
    let range = r(CivilDateTime::MIN, end, 1, TemporalUnit::Days);
    assert_eq!(
        range.len(),
        (end.date().day_number() - CivilDateTime::MIN.date().day_number()) as u128
    );
    assert!(range.contains(&CivilDateTime::MIN));
    assert!(!range.contains(&end));
}
#[test]
fn full_year_range() {
    let range = r(CalendarYear::MIN, CalendarYear::MAX, 1, TemporalUnit::Years);
    assert_eq!(
        range.len(),
        (CalendarYear::MAX.value() - CalendarYear::MIN.value()) as u128
    );
    assert!(range.contains(&CalendarYear::MIN));
    assert!(!range.contains(&CalendarYear::MAX));
}
#[test]
fn invalid_increment_unit_amount() {
    assert_eq!(
        TemporalValueRange::new(CivilDate::MIN, CivilDate::MAX, 0, TemporalUnit::Days),
        Err(TemporalRangeError::NonPositiveStep)
    );
}
#[test]
fn from_after_to() {
    assert_eq!(
        TemporalValueRange::new(CivilDate::MAX, CivilDate::MIN, 1, TemporalUnit::Days),
        Err(TemporalRangeError::DescendingBounds)
    );
}
#[test]
fn remainder_on_increment_amount() {
    assert_eq!(
        TemporalValueRange::new(
            CalendarYear::new(0).unwrap(),
            CalendarYear::new(3).unwrap(),
            2,
            TemporalUnit::Years
        ),
        Err(TemporalRangeError::UnalignedEnd)
    );
}
#[test]
fn remainder_on_increment_type() {
    assert_eq!(
        TemporalValueRange::new(
            CivilTime::new(11, 30, 0, 0).unwrap(),
            CivilTime::new(13, 29, 0, 0).unwrap(),
            1,
            TemporalUnit::Hours
        ),
        Err(TemporalRangeError::UnalignedEnd)
    );
}
#[test]
fn remainder_on_increment_type_exceeds_maximum_year() {
    assert_ne!(
        (CalendarYear::MAX.value() - CalendarYear::MIN.value()) % 10,
        0
    );
    assert_eq!(
        TemporalValueRange::new(
            CalendarYear::MIN,
            CalendarYear::MAX,
            1,
            TemporalUnit::Decades
        ),
        Err(TemporalRangeError::UnalignedEnd)
    );
}
#[test]
fn get_index_greater_than_size() {
    let range = r(
        CalendarYear::new(0).unwrap(),
        CalendarYear::new(1).unwrap(),
        1,
        TemporalUnit::Years,
    );
    assert_eq!(range.len(), 1);
    assert_eq!(range.get(1), None);
}

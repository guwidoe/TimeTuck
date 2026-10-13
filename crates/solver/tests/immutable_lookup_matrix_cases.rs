// Complete pinned Community LookupStrategyImmutableTest/data adaptation, Apache-2.0.
// Native immutable values/component records; no calendar arithmetic or Java ABI claim.
use std::{
    fmt::Debug,
    sync::Arc,
    time::{Duration, SystemTime},
};
use timetuck::*;
#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalDate {
    year: i32,
    month: u8,
    day: u8,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalTime {
    hour: u8,
    minute: u8,
    second: u8,
    nanos: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct LocalDateTime {
    date: LocalDate,
    time: LocalTime,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct MonthDay(u8, u8);
#[derive(Clone, Copy, Debug, PartialEq)]
struct DayOfWeek(u8);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Month(u8);
#[derive(Clone, Copy, Debug, PartialEq)]
struct YearMonth(i32, u8);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Year(i32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct ZoneOffset {
    seconds: i32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct OffsetDateTime {
    datetime: LocalDateTime,
    offset: ZoneOffset,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct OffsetTime {
    time: LocalTime,
    offset: ZoneOffset,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct ZonedDateTime {
    datetime: LocalDateTime,
    zone: ZoneOffset,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Period {
    years: i32,
    months: i32,
    days: i32,
}
#[derive(Clone, Copy)]
enum Mode {
    Add,
    Remove,
    Lookup,
}
fn row<T: Debug + PartialEq>(internal: T, external: T, mode: Mode) {
    let mut lookup = WorkingObjectLookup::<T, usize>::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::Immutable,
    );
    let internal = Arc::new(internal);
    let external = Arc::new(external);
    match mode {
        Mode::Add => lookup.add(internal).unwrap(),
        Mode::Remove => lookup.remove(&internal).unwrap(),
        Mode::Lookup => {
            let found = lookup.lookup(&external).unwrap();
            assert_eq!(found, internal);
            assert!(Arc::ptr_eq(&found, &external));
        }
    }
}
fn all_rows(mode: Mode) {
    row(true, true, mode);
    row(1i8, 1i8, mode);
    row(1i16, 1i16, mode);
    row(1i32, 1i32, mode);
    row(1i64, 1i64, mode);
    row(0.5f32, 0.5f32, mode);
    row(0.1f64, 0.1f64, mode);
    row(
        num_bigint::BigInt::from(1),
        "1".parse::<num_bigint::BigInt>().unwrap(),
        mode,
    );
    row(Decimal::from(1), "1".parse::<Decimal>().unwrap(), mode);
    row('!', '\u{21}', mode);
    row(String::new(), String::new(), mode);
    row(
        SystemTime::UNIX_EPOCH + Duration::from_millis(12345),
        SystemTime::UNIX_EPOCH + Duration::from_millis(12345),
        mode,
    );
    let date = LocalDate {
        year: 1,
        month: 2,
        day: 3,
    };
    let minute = LocalTime {
        hour: 4,
        minute: 5,
        second: 0,
        nanos: 0,
    };
    let datetime = LocalDateTime { date, time: minute };
    row(
        datetime,
        LocalDateTime {
            date: LocalDate {
                year: 1,
                month: 2,
                day: 3,
            },
            time: LocalTime {
                hour: 4,
                minute: 5,
                second: 0,
                nanos: 0,
            },
        },
        mode,
    );
    row(
        LocalTime {
            hour: 1,
            minute: 2,
            second: 0,
            nanos: 0,
        },
        LocalTime {
            hour: 1,
            minute: 2,
            second: 0,
            nanos: 0,
        },
        mode,
    );
    row(
        date,
        LocalDate {
            year: 1,
            month: 2,
            day: 3,
        },
        mode,
    );
    row(MonthDay(12, 31), MonthDay(12, 31), mode);
    row(DayOfWeek(1), DayOfWeek(1), mode);
    row(Month(12), Month(12), mode);
    row(YearMonth(1999, 12), YearMonth(1999, 12), mode);
    row(Year(1999), Year(1999), mode);
    let utc = ZoneOffset { seconds: 0 };
    let full = LocalDateTime {
        date,
        time: LocalTime {
            hour: 4,
            minute: 5,
            second: 6,
            nanos: 7,
        },
    };
    row(
        OffsetDateTime {
            datetime: full,
            offset: utc,
        },
        OffsetDateTime {
            datetime: LocalDateTime {
                date: LocalDate {
                    year: 1,
                    month: 2,
                    day: 3,
                },
                time: LocalTime {
                    hour: 4,
                    minute: 5,
                    second: 6,
                    nanos: 7,
                },
            },
            offset: ZoneOffset { seconds: 0 },
        },
        mode,
    );
    row(
        OffsetTime {
            time: LocalTime {
                hour: 1,
                minute: 2,
                second: 3,
                nanos: 4,
            },
            offset: utc,
        },
        OffsetTime {
            time: LocalTime {
                hour: 1,
                minute: 2,
                second: 3,
                nanos: 4,
            },
            offset: ZoneOffset { seconds: 0 },
        },
        mode,
    );
    row(
        ZonedDateTime {
            datetime: full,
            zone: utc,
        },
        ZonedDateTime {
            datetime: LocalDateTime {
                date: LocalDate {
                    year: 1,
                    month: 2,
                    day: 3,
                },
                time: LocalTime {
                    hour: 4,
                    minute: 5,
                    second: 6,
                    nanos: 7,
                },
            },
            zone: ZoneOffset { seconds: 0 },
        },
        mode,
    );
    row(utc, ZoneOffset { seconds: 0 }, mode);
    row(
        Duration::from_secs(5 * 24 * 60 * 60),
        Duration::from_secs(5 * 24 * 60 * 60),
        mode,
    );
    row(
        Period {
            years: 1,
            months: 2,
            days: 3,
        },
        Period {
            years: 1,
            months: 2,
            days: 3,
        },
        mode,
    );
}
#[test]
fn add_immutable() {
    all_rows(Mode::Add)
}
#[test]
fn remove_immutable() {
    all_rows(Mode::Remove)
}
#[test]
fn look_up_immutable() {
    all_rows(Mode::Lookup)
}

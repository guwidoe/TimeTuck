//! Original typed construction, actual Gregorian/offset/fraction boundaries and codec controls.
use std::{cell::Cell, time::Duration};
use timetuck::*;
#[test]
fn calendar_offset_fraction_round_trips_and_real_departures() {
    for input in [
        "0000-02-29T00:00:00Z",
        "2000-02-29T23:59:59.123456789+18:00",
        "2024-02-29T01:02:03.000000001-18:00",
        "2025-12-31T23:59:59.1+05:45",
        "1900-02-28T12:30:01-00:30:15",
    ] {
        let t: JsonOffsetDateTime = input.parse().unwrap();
        assert_eq!(t.to_string().parse::<JsonOffsetDateTime>().unwrap(), t);
        assert_eq!(t.departure().offset_seconds(), t.offset_seconds());
    }
    for input in [
        "1900-02-29T12:00:00Z",
        "2100-02-29T12:00:00Z",
        "2024-04-31T12:00:00Z",
        "2024-00-01T12:00:00Z",
        "2024-01-00T12:00:00Z",
        "2024-01-01T24:00:00Z",
        "2024-01-01T23:60:00Z",
        "2024-01-01T23:59:60Z",
        "2024-01-01T00:00:00+18:01",
        "2024-01-01T00:00:00.1234567890Z",
        "2024-01-01T00:00:00Zjunk",
        "2024-01-01T00:00:00+02:60",
        "2024-01-01T00:00:00",
        "2024-01-01T00:00:00é",
    ] {
        assert!(input.parse::<JsonOffsetDateTime>().is_err(), "{input}");
    }
    let zero: JsonOffsetDateTime = "1970-01-01T00:00:00Z".parse().unwrap();
    assert_eq!(zero.departure().unix_seconds(), 0);
    let before: JsonOffsetDateTime = "1969-12-31T23:59:59Z".parse().unwrap();
    assert_eq!(before.departure().unix_seconds(), -1);
    let offset: JsonOffsetDateTime = "1970-01-01T00:30:00+01:00".parse().unwrap();
    assert_eq!(offset.departure().unix_seconds(), -1800);
    let next: JsonOffsetDateTime = "2000-03-01T00:00:00Z".parse().unwrap();
    let previous: JsonOffsetDateTime = "2000-02-28T00:00:00Z".parse().unwrap();
    assert_eq!(
        next.departure().unix_seconds() - previous.departure().unix_seconds(),
        2 * 86400
    );
    assert!(JsonOffsetDateTime::new((2023, 2, 29), (0, 0, 0, 0), 0).is_err());
    assert!(JsonOffsetDateTime::new((2024, 2, 29), (0, 0, 0, 1_000_000_000), 0).is_err());
    assert!(JsonOffsetDateTime::new((2024, 1, 1), (0, 0, 0, 0), 64801).is_err());
}
#[test]
fn signed_duration_parsing_fraction_order_overflow_and_std() {
    for input in [
        "PT0S",
        "PT5H30M",
        "P2DT3H4M5.000000001S",
        "-PT0.1S",
        "PT-1H2M",
        "+PT5S",
        "PT100000H",
    ] {
        let d: JsonDuration = input.parse().unwrap();
        assert_eq!(d.to_string().parse::<JsonDuration>().unwrap(), d);
    }
    assert_eq!(
        "P2DT3H4M5.000000001S"
            .parse::<JsonDuration>()
            .unwrap()
            .nanoseconds(),
        (2 * 86400 + 3 * 3600 + 4 * 60 + 5) * 1_000_000_000 + 1
    );
    assert_eq!(
        "PT-1H2M".parse::<JsonDuration>().unwrap().nanoseconds(),
        -3480 * 1_000_000_000
    );
    for input in [
        "P",
        "PT",
        "P1DT",
        "PT1M2H",
        "PT1H1H",
        "PT1.5H",
        "PT.5S",
        "PT1.S",
        "PT0.1234567890S",
        "P1Y",
        "P1M",
        "P1W",
        "PT99999999999999999999999999999999999999999S",
    ] {
        assert!(input.parse::<JsonDuration>().is_err(), "{input}");
    }
    for d in [
        Duration::ZERO,
        Duration::new(19800, 123),
        Duration::new(u64::MAX, 999_999_999),
    ] {
        let native = JsonDuration::from_std(d);
        assert_eq!(native.to_std().unwrap(), d);
        assert_eq!(native.to_string().parse::<JsonDuration>().unwrap(), native);
    }
    assert!("-PT1S".parse::<JsonDuration>().unwrap().to_std().is_err());
    assert!(JsonDuration::from_nanoseconds(i128::MIN).is_err());
}
#[derive(Default)]
struct Record {
    id: Option<String>,
    time: Option<JsonOffsetDateTime>,
    duration: Option<JsonDuration>,
}
#[test]
fn strict_records_validate_before_callbacks_escape_keys_and_round_trip() {
    let calls = Cell::new(0);
    let mut c = TypedJsonRecordCodec::new(|| {
        calls.set(calls.get() + 1);
        Record::default()
    });
    c.field(
        "id",
        |s| s.id.as_ref(),
        |s, v| {
            calls.set(calls.get() + 1);
            s.id = v;
        },
    )
    .unwrap();
    c.field(
        "t\"ime",
        |s| s.time.as_ref(),
        |s, v| {
            calls.set(calls.get() + 1);
            s.time = v;
        },
    )
    .unwrap();
    c.field(
        "duration",
        |s| s.duration.as_ref(),
        |s, v| {
            calls.set(calls.get() + 1);
            s.duration = v;
        },
    )
    .unwrap();
    assert!(matches!(
        c.field::<String>("id", |s| s.id.as_ref(), |s, v| s.id = v),
        Err(TypedJsonRecordError::DuplicateField(_))
    ));
    for input in [
        r#"{"id":"123","unknown\u0050roperty":"x"}"#,
        r#"{"id":"123","t\"ime":"2023-02-29T12:00:00Z"}"#,
        r#"{"id":3}"#,
        r#"[]"#,
        r#"{"id":}"#,
    ] {
        assert!(c.decode(input).is_err());
        assert_eq!(calls.get(), 0);
    }
    let input = r#"{"\u0069d":"a\"b\\c\n","t\"ime":"2024-02-29T23:59:59.001-03:30","duration":"P1DT2H3M4.5S"}"#;
    let a = c.decode(input).unwrap();
    let json = c.encode(&a).unwrap();
    let b = c.decode(&json).unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(a.time, b.time);
    assert_eq!(a.duration, b.duration);
    assert!(json.contains(r#""t\"ime""#));
    let empty = c
        .decode(r#"{"id":null,"t\"ime":null,"duration":null}"#)
        .unwrap();
    assert_eq!(c.encode(&empty).unwrap(), "{}");
}
#[test]
fn typed_date_time_schema_rejects_unsupported_keywords_and_invalid_values() {
    let v = OffsetDateTimeJsonValidator::read_schema(r#"{"type":"string","format":"date-time"}"#)
        .unwrap();
    for value in [
        serde_json::json!("2023-02-29T12:00:00Z"),
        serde_json::json!("2024-01-01T12:00:00+19:00"),
        serde_json::json!(3),
        serde_json::Value::Null,
    ] {
        assert_eq!(v.validate(&value).len(), 1);
    }
    for schema in [
        r#"{"type":"string","format":"email"}"#,
        r#"{"type":"string","format":"date-time","pattern":".*"}"#,
        r#"{"type":"integer","format":"date-time"}"#,
        r#"{"type":"string","allOf":[]}"#,
    ] {
        assert!(OffsetDateTimeJsonValidator::read_schema(schema).is_err());
    }
}

// Full adapted pinned Community TEST assertions, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/sdk_time_fixture.rs"]
mod fixture;
use fixture::*;
use std::time::Duration;
use timetuck::*;
#[test]
fn deserialization_of_unknown_properties_throws_exception() {
    let input = r#"{
"id":"123",
"unknownProperty":"shouldFail"
}"#;
    let e = match simple_codec().decode(input) {
        Err(e) => e,
        Ok(_) => panic!("unknown property accepted"),
    };
    assert!(matches!(e, TypedJsonRecordError::UnknownProperty(_)));
    assert!(e.to_string().contains("unknownProperty"));
}
#[test]
fn serialize_non_null_properties_only_includes_non_null() {
    assert_eq!(simple_codec().encode(&Simple { id: None }).unwrap(), "{}");
}
#[test]
fn serialize_and_deserialize_date_time_preserves_time_zone() {
    let c = date_time_codec();
    let dto = c
        .decode(r#"{"dateTime":"2024-06-01T12:00:00+02:00"}"#)
        .unwrap();
    assert_eq!(dto.date_time.unwrap().offset_seconds(), 7200);
    assert!(
        c.encode(&dto)
            .unwrap()
            .contains(r#""dateTime":"2024-06-01T12:00:00+02:00""#)
    );
}
#[test]
fn serialize_offset_date_time_as_string() {
    let dto = DateTimeWrapper {
        date_time: Some(JsonOffsetDateTime::new((2025, 1, 1), (10, 0, 0, 0), 0).unwrap()),
    };
    let expected = r#"{"dateTime":"2025-01-01T10:00:00Z"}"#;
    assert_eq!(date_time_codec().encode(&dto).unwrap(), expected);
}
#[test]
fn serialize_duration_as_string() {
    let dto = DurationWrapper {
        duration: Some(JsonDuration::from_std(Duration::from_secs(
            5 * 3600 + 30 * 60,
        ))),
    };
    assert_eq!(
        duration_codec().encode(&dto).unwrap(),
        r#"{"duration":"PT5H30M"}"#
    );
}
#[test]
fn valid_offset_date_time_format() {
    let validator = OffsetDateTimeJsonValidator::read_schema(SCHEMA).unwrap();
    assert!(
        validator
            .validate(&serde_json::Value::String("2027-02-01T17:00:00Z".into()))
            .is_empty()
    );
}
#[test]
fn invalid_offset_date_time_format() {
    let validator = OffsetDateTimeJsonValidator::read_schema(SCHEMA).unwrap();
    assert_eq!(
        validator.validate(&serde_json::Value::String("2027-02-01 17:00:00Z".into())),
        vec![
            "$: does not match the date-time pattern must be a valid ISO-8601 date and time with an offset"
        ]
    );
}

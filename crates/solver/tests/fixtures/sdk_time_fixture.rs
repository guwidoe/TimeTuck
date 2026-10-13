// Adapted SDK mapper record and ValidatorTest schema TEST fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Default)]
pub struct Simple {
    pub id: Option<String>,
}
#[derive(Default)]
pub struct DateTimeWrapper {
    pub date_time: Option<JsonOffsetDateTime>,
}
#[derive(Default)]
pub struct DurationWrapper {
    pub duration: Option<JsonDuration>,
}
pub fn simple_codec() -> TypedJsonRecordCodec<'static, Simple> {
    let mut c = TypedJsonRecordCodec::new(Simple::default);
    c.field("id", |s| s.id.as_ref(), |s, v| s.id = v).unwrap();
    c
}
pub fn date_time_codec() -> TypedJsonRecordCodec<'static, DateTimeWrapper> {
    let mut c = TypedJsonRecordCodec::new(DateTimeWrapper::default);
    c.field("dateTime", |s| s.date_time.as_ref(), |s, v| s.date_time = v)
        .unwrap();
    c
}
pub fn duration_codec() -> TypedJsonRecordCodec<'static, DurationWrapper> {
    let mut c = TypedJsonRecordCodec::new(DurationWrapper::default);
    c.field("duration", |s| s.duration.as_ref(), |s, v| s.duration = v)
        .unwrap();
    c
}
pub const SCHEMA: &str = r#"{
"description":"ISO 8601 datetime with offset to UTC)",
"type":"string",
"allOf":[{"format":"date-time","type":"string"}]
}"#;

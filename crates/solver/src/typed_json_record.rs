//! Optional strict typed record codec, using actual caller constructors/accessors.
use crate::{JsonDuration, JsonOffsetDateTime};
use serde_json::{Map, Value};
use std::{any::Any, error::Error, fmt};
#[derive(Debug)]
pub enum TypedJsonRecordError {
    Json(serde_json::Error),
    NotObject,
    UnknownProperty(String),
    DuplicateField(String),
    Field { field: String, message: String },
    UnsupportedSchema(String),
}
impl fmt::Display for TypedJsonRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => e.fmt(f),
            Self::UnknownProperty(name) => write!(f, "unknown property {name:?}"),
            Self::Field { field, message } => write!(f, "property {field:?}: {message}"),
            _ => write!(f, "typed JSON record: {self:?}"),
        }
    }
}
impl Error for TypedJsonRecordError {}
impl From<serde_json::Error> for TypedJsonRecordError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
/// Scalar conversion supplies actual typed record fields. Optional/null storage
/// belongs to the codec. This is not an arbitrary JSON Schema/Jackson substitute.
pub trait JsonRecordScalar: 'static {
    fn from_json(value: &Value) -> Result<Self, String>
    where
        Self: Sized;
    fn to_json(&self) -> Value;
}
impl JsonRecordScalar for String {
    fn from_json(v: &Value) -> Result<Self, String> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| "expected string".into())
    }
    fn to_json(&self) -> Value {
        Value::String(self.clone())
    }
}
macro_rules! temporal_scalar {
    ($t:ty) => {
        impl JsonRecordScalar for $t {
            fn from_json(v: &Value) -> Result<Self, String> {
                v.as_str()
                    .ok_or_else(|| "expected ISO temporal string".to_string())?
                    .parse::<Self>()
                    .map_err(|e| e.to_string())
            }
            fn to_json(&self) -> Value {
                Value::String(self.to_string())
            }
        }
    };
}
temporal_scalar!(JsonOffsetDateTime);
temporal_scalar!(JsonDuration);
type ReadField<'a, T> = dyn Fn(&T) -> Option<Value> + 'a;
type DecodeField<'a> = dyn Fn(&Value) -> Result<Box<dyn Any>, String> + 'a;
type WriteField<'a, T> = dyn Fn(&mut T, Box<dyn Any>) + 'a;
struct Field<'a, T> {
    name: String,
    read: Box<ReadField<'a, T>>,
    decode: Box<DecodeField<'a>>,
    write: Box<WriteField<'a, T>>,
}
/// Unknown fields and every field value validate before the constructor or any
/// setter runs. Reads serialize live fields and omit None; setters receive fully
/// parsed typed values. Missing fields retain constructor defaults, explicit null
/// writes None. JSON duplicate keys follow serde_json's last-value policy.
/// Callback effects/panics are caller-owned; no Java record/reflection ABI inferred.
pub struct TypedJsonRecordCodec<'a, T> {
    create: Box<dyn Fn() -> T + 'a>,
    fields: Vec<Field<'a, T>>,
}
impl<'a, T> TypedJsonRecordCodec<'a, T> {
    pub fn new(create: impl Fn() -> T + 'a) -> Self {
        Self {
            create: Box::new(create),
            fields: vec![],
        }
    }
    pub fn field<S: JsonRecordScalar>(
        &mut self,
        name: impl Into<String>,
        read: impl for<'t> Fn(&'t T) -> Option<&'t S> + 'a,
        write: impl Fn(&mut T, Option<S>) + 'a,
    ) -> Result<&mut Self, TypedJsonRecordError> {
        let name = name.into();
        if self.fields.iter().any(|f| f.name == name) {
            return Err(TypedJsonRecordError::DuplicateField(name));
        }
        self.fields.push(Field {
            name,
            read: Box::new(move |t| read(t).map(JsonRecordScalar::to_json)),
            decode: Box::new(|v| {
                let value = if v.is_null() {
                    None
                } else {
                    Some(S::from_json(v)?)
                };
                Ok(Box::new(value) as Box<dyn Any>)
            }),
            write: Box::new(move |t, value| {
                write(
                    t,
                    *value
                        .downcast::<Option<S>>()
                        .expect("internal typed decoder invariant"),
                )
            }),
        });
        Ok(self)
    }
    pub fn decode(&self, input: &str) -> Result<T, TypedJsonRecordError> {
        let value: Value = serde_json::from_str(input)?;
        let object = value.as_object().ok_or(TypedJsonRecordError::NotObject)?;
        for name in object.keys() {
            if !self.fields.iter().any(|f| &f.name == name) {
                return Err(TypedJsonRecordError::UnknownProperty(name.clone()));
            }
        }
        let mut parsed = vec![];
        for (i, field) in self.fields.iter().enumerate() {
            if let Some(v) = object.get(&field.name) {
                let v = (field.decode)(v).map_err(|message| TypedJsonRecordError::Field {
                    field: field.name.clone(),
                    message,
                })?;
                parsed.push((i, v));
            }
        }
        let mut record = (self.create)();
        for (i, value) in parsed {
            (self.fields[i].write)(&mut record, value);
        }
        Ok(record)
    }
    pub fn encode(&self, record: &T) -> Result<String, TypedJsonRecordError> {
        let mut object = Map::new();
        for field in &self.fields {
            if let Some(v) = (field.read)(record) {
                object.insert(field.name.clone(), v);
            }
        }
        Ok(serde_json::to_string(&object)?)
    }
}
/// Narrow native validator for a declared string/date-time format, directly or
/// under allOf. Description is accepted; every other keyword/format is rejected.
/// Validation uses the actual calendar/offset parser, not a fixture pattern flag.
#[derive(Clone, Copy, Debug)]
pub struct OffsetDateTimeJsonValidator;
impl OffsetDateTimeJsonValidator {
    pub fn read_schema(input: &str) -> Result<Self, TypedJsonRecordError> {
        fn check(value: &Value) -> Result<bool, TypedJsonRecordError> {
            let o = value
                .as_object()
                .ok_or_else(|| TypedJsonRecordError::UnsupportedSchema("expected object".into()))?;
            for key in o.keys() {
                if !["description", "type", "format", "allOf"].contains(&key.as_str()) {
                    return Err(TypedJsonRecordError::UnsupportedSchema(key.clone()));
                }
            }
            if o.get("type").and_then(Value::as_str) != Some("string") {
                return Err(TypedJsonRecordError::UnsupportedSchema(
                    "expected string type".into(),
                ));
            }
            let mut format = false;
            if let Some(v) = o.get("format") {
                if v.as_str() != Some("date-time") {
                    return Err(TypedJsonRecordError::UnsupportedSchema(
                        "unsupported format".into(),
                    ));
                }
                format = true;
            }
            if let Some(v) = o.get("allOf") {
                for child in v.as_array().ok_or_else(|| {
                    TypedJsonRecordError::UnsupportedSchema("expected allOf array".into())
                })? {
                    format |= check(child)?;
                }
            }
            Ok(format)
        }
        let value: Value = serde_json::from_str(input)?;
        if !check(&value)? {
            return Err(TypedJsonRecordError::UnsupportedSchema(
                "date-time format missing".into(),
            ));
        }
        Ok(Self)
    }
    pub fn validate(&self, value: &Value) -> Vec<String> {
        if value
            .as_str()
            .is_some_and(|s| s.parse::<JsonOffsetDateTime>().is_ok())
        {
            vec![]
        } else {
            vec!["$: does not match the date-time pattern must be a valid ISO-8601 date and time with an offset".into()]
        }
    }
}

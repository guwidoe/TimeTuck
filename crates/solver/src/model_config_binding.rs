//! Explicit typed configuration members, their metadata and executable JSON binding.
use crate::{ConstraintWeightOverrides, Decimal, JsonDuration};
use serde_json::{Map, Value, json};
use std::{any::Any, collections::BTreeMap, error::Error, fmt, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelConfigParameterKind {
    Weight,
    Parameter,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelConfigSchemaType {
    Integer,
    Number,
    String,
    Array,
    Object,
    Boolean,
}
impl ModelConfigSchemaType {
    fn name(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Number => "number",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
            Self::Boolean => "boolean",
        }
    }
    fn parse(value: &str) -> Result<Self, ModelConfigBindingError> {
        match value {
            "integer" => Ok(Self::Integer),
            "number" => Ok(Self::Number),
            "string" => Ok(Self::String),
            "array" => Ok(Self::Array),
            "object" => Ok(Self::Object),
            "boolean" => Ok(Self::Boolean),
            _ => Err(failure("unknown configuration schema type")),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelConfigDataFormat {
    Duration,
    Number,
    Percentage,
}
impl ModelConfigDataFormat {
    fn name(self) -> &'static str {
        match self {
            Self::Duration => "duration",
            Self::Number => "number",
            Self::Percentage => "percentage",
        }
    }
    fn parse(value: &str) -> Result<Self, ModelConfigBindingError> {
        match value {
            "duration" => Ok(Self::Duration),
            "number" => Ok(Self::Number),
            "percentage" => Ok(Self::Percentage),
            _ => Err(failure("unknown configuration format")),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelConfigBindingError(pub String);
impl fmt::Display for ModelConfigBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ModelConfigBindingError {}
fn failure(message: impl Into<String>) -> ModelConfigBindingError {
    ModelConfigBindingError(message.into())
}
fn nonempty(value: &str, label: &str) -> Result<(), ModelConfigBindingError> {
    if value.trim().is_empty() {
        Err(failure(format!("{label} must be nonblank")))
    } else {
        Ok(())
    }
}

/// Shapes are supplied by the actual value codec. Arrays and optional values
/// compose that codec; named objects explicitly declare their nested properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelConfigValueShape {
    pub schema_type: ModelConfigSchemaType,
    pub schema_type_ref: Option<String>,
    pub format: Option<ModelConfigDataFormat>,
    pub nullable: bool,
    pub items: Option<Box<Self>>,
    pub array_length: Option<usize>,
    pub properties: BTreeMap<String, Self>,
    pub required: Vec<String>,
}
impl ModelConfigValueShape {
    pub fn scalar(schema_type: ModelConfigSchemaType) -> Self {
        Self {
            schema_type,
            schema_type_ref: None,
            format: None,
            nullable: false,
            items: None,
            array_length: None,
            properties: BTreeMap::new(),
            required: vec![],
        }
    }
    pub fn object(name: impl Into<String>, properties: BTreeMap<String, Self>) -> Self {
        Self {
            schema_type_ref: Some(name.into()),
            required: properties.keys().cloned().collect(),
            properties,
            ..Self::scalar(ModelConfigSchemaType::Object)
        }
    }
    fn validate(&self) -> Result<(), ModelConfigBindingError> {
        if let Some(name) = &self.schema_type_ref {
            nonempty(name, "schema reference")?;
        }
        if (self.schema_type == ModelConfigSchemaType::Array) != self.items.is_some()
            || (self.schema_type != ModelConfigSchemaType::Array && self.array_length.is_some())
            || (self.schema_type != ModelConfigSchemaType::Object && !self.properties.is_empty())
            || self
                .required
                .iter()
                .any(|name| !self.properties.contains_key(name))
        {
            return Err(failure("configuration codec shape is inconsistent"));
        }
        if self.format == Some(ModelConfigDataFormat::Duration)
            && self.schema_type != ModelConfigSchemaType::String
        {
            return Err(failure("duration format requires a string codec"));
        }
        if let Some(items) = &self.items {
            items.validate()?;
        }
        for (name, shape) in &self.properties {
            nonempty(name, "object member")?;
            shape.validate()?;
        }
        Ok(())
    }
    fn reference(&self) -> Option<&str> {
        self.schema_type_ref
            .as_deref()
            .or_else(|| self.items.as_deref().and_then(Self::reference))
    }
    fn schema(
        &self,
        definitions: &mut BTreeMap<String, Value>,
    ) -> Result<Value, ModelConfigBindingError> {
        let mut schema = match self.schema_type {
            ModelConfigSchemaType::Array => json!({
                "type":"array",
                "items":self.items.as_ref().expect("validated shape").schema(definitions)?
            }),
            ModelConfigSchemaType::Object => {
                let mut properties = Map::new();
                for (name, shape) in &self.properties {
                    properties.insert(name.clone(), shape.schema(definitions)?);
                }
                let body = json!({"type":"object", "properties":properties, "required":self.required, "additionalProperties":false});
                if let Some(name) = &self.schema_type_ref {
                    if definitions.get(name).is_some_and(|v| v != &body) {
                        return Err(failure(format!("conflicting object schema {name:?}")));
                    }
                    definitions.insert(name.clone(), body);
                    json!({"$ref":format!("#/$defs/{}", pointer_segment(name))})
                } else {
                    body
                }
            }
            _ => json!({"type":self.schema_type.name()}),
        };
        if let Some(format) = self.format {
            schema["format"] = json!(format.name());
        }
        if let Some(length) = self.array_length {
            schema["minItems"] = json!(length);
            schema["maxItems"] = json!(length);
        }
        if self.schema_type != ModelConfigSchemaType::Object
            && let Some(name) = &self.schema_type_ref
        {
            if definitions.get(name).is_some_and(|v| v != &schema) {
                return Err(failure(format!("conflicting scalar schema {name:?}")));
            }
            definitions.insert(name.clone(), schema);
            schema = json!({"$ref":format!("#/$defs/{}", pointer_segment(name))});
        }
        if self.nullable {
            schema = json!({"anyOf":[schema, {"type":"null"}]});
        }
        Ok(schema)
    }
}
fn pointer_segment(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// Custom codecs own their domain validation and must faithfully describe their
/// wire values. Conversion failures propagate before any constructor/setter runs.
pub trait ModelConfigValue: Sized + 'static {
    fn shape() -> ModelConfigValueShape;
    fn from_config_json(value: &Value) -> Result<Self, String>;
    fn to_config_json(&self) -> Result<Value, String>;
}
macro_rules! integer_value {
    ($($ty:ty),+ $(,)?) => {$ (
        impl ModelConfigValue for $ty {
            fn shape() -> ModelConfigValueShape { ModelConfigValueShape::scalar(ModelConfigSchemaType::Integer) }
            fn from_config_json(value: &Value) -> Result<Self, String> {
                value.as_i64().and_then(|v| Self::try_from(v).ok()).ok_or_else(|| "expected integer in native range".into())
            }
            fn to_config_json(&self) -> Result<Value, String> { Ok(json!(self)) }
        }
    )+};
}
integer_value!(i64, i32);
impl ModelConfigValue for String {
    fn shape() -> ModelConfigValueShape {
        ModelConfigValueShape::scalar(ModelConfigSchemaType::String)
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "expected string".into())
    }
    fn to_config_json(&self) -> Result<Value, String> {
        Ok(json!(self))
    }
}
impl ModelConfigValue for bool {
    fn shape() -> ModelConfigValueShape {
        ModelConfigValueShape::scalar(ModelConfigSchemaType::Boolean)
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        value.as_bool().ok_or_else(|| "expected boolean".into())
    }
    fn to_config_json(&self) -> Result<Value, String> {
        Ok(json!(self))
    }
}
impl ModelConfigValue for JsonDuration {
    fn shape() -> ModelConfigValueShape {
        let mut shape = ModelConfigValueShape::scalar(ModelConfigSchemaType::String);
        shape.schema_type_ref = Some("Duration".into());
        shape.format = Some(ModelConfigDataFormat::Duration);
        shape
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        value
            .as_str()
            .ok_or_else(|| "expected ISO duration string".to_owned())?
            .parse::<Self>()
            .map_err(|e| e.to_string())
    }
    fn to_config_json(&self) -> Result<Value, String> {
        Ok(json!(self.to_string()))
    }
}
impl<V: ModelConfigValue> ModelConfigValue for Option<V> {
    fn shape() -> ModelConfigValueShape {
        let mut shape = V::shape();
        shape.nullable = true;
        shape
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        if value.is_null() {
            Ok(None)
        } else {
            V::from_config_json(value).map(Some)
        }
    }
    fn to_config_json(&self) -> Result<Value, String> {
        self.as_ref()
            .map_or(Ok(Value::Null), ModelConfigValue::to_config_json)
    }
}
impl<V: ModelConfigValue> ModelConfigValue for Vec<V> {
    fn shape() -> ModelConfigValueShape {
        let mut shape = ModelConfigValueShape::scalar(ModelConfigSchemaType::Array);
        shape.items = Some(Box::new(V::shape()));
        shape
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        value
            .as_array()
            .ok_or_else(|| "expected array".to_owned())?
            .iter()
            .map(V::from_config_json)
            .collect()
    }
    fn to_config_json(&self) -> Result<Value, String> {
        self.iter()
            .map(ModelConfigValue::to_config_json)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }
}
impl<V: ModelConfigValue, const N: usize> ModelConfigValue for [V; N] {
    fn shape() -> ModelConfigValueShape {
        let mut shape = Vec::<V>::shape();
        shape.array_length = Some(N);
        shape
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        Vec::<V>::from_config_json(value)?
            .try_into()
            .map_err(|_| format!("expected array of length {N}"))
    }
    fn to_config_json(&self) -> Result<Value, String> {
        self.iter()
            .map(ModelConfigValue::to_config_json)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }
}
fn decimal_text(text: &str) -> Result<Decimal, String> {
    text.parse::<Decimal>().map_err(|e| e.to_string())
}
impl ModelConfigValue for Decimal {
    fn shape() -> ModelConfigValueShape {
        ModelConfigValueShape::scalar(ModelConfigSchemaType::Number)
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        let n = value.as_number().ok_or("expected decimal number")?;
        decimal_text(&n.to_string())
    }
    fn to_config_json(&self) -> Result<Value, String> {
        let value: Value = serde_json::from_str(&self.to_string()).map_err(|e| e.to_string())?;
        let decoded = Self::from_config_json(&value)?;
        if self.numeric_cmp(&decoded).is_ne() {
            return Err("decimal cannot be represented exactly by this JSON backend".into());
        }
        Ok(value)
    }
}
// serde_json without arbitrary_precision can shorten a decimal number. Reject
// those inputs instead of silently changing their mathematical value. Strings
// (including escaped quotes) are skipped. Syntax was validated by serde_json.
fn check_numeric_lexemes(input: &str) -> Result<(), ModelConfigBindingError> {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
        } else if bytes[i] == b'-' || bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || b".eE+-".contains(&bytes[i])) {
                i += 1;
            }
            let token = &input[start..i];
            let value: Value = serde_json::from_str(token).map_err(|e| failure(e.to_string()))?;
            let exact = decimal_text(token).map_err(failure)?;
            let parsed = Decimal::from_config_json(&value).map_err(failure)?;
            if exact.numeric_cmp(&parsed).is_ne() {
                return Err(failure("JSON number loses decimal precision"));
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Metadata customizations do not override the actual codec's structural type.
/// A reference defaults to a weight role; parameters explicitly opt out. Weight
/// roles without a reference use their external member ID as the native target.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelConfigMemberOptions {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub kind: Option<ModelConfigParameterKind>,
    pub format: Option<ModelConfigDataFormat>,
    pub nullable: Option<bool>,
    pub constraint_name_ref: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelConfigParameter {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: ModelConfigParameterKind,
    pub schema_type: ModelConfigSchemaType,
    pub array_item_type: Option<ModelConfigSchemaType>,
    pub schema_type_ref: Option<String>,
    pub format: Option<ModelConfigDataFormat>,
    pub nullable: bool,
    pub constraint_name_ref: Option<String>,
}
impl ModelConfigParameter {
    fn value(&self) -> Value {
        json!({"id":self.id,"name":self.name,"description":self.description,
            "kind":if self.kind == ModelConfigParameterKind::Weight {"WEIGHT"} else {"PARAMETER"},
            "type":self.schema_type.name(),"arrayItemType":self.array_item_type.map(ModelConfigSchemaType::name),
            "schemaTypeRef":self.schema_type_ref,"format":self.format.map(ModelConfigDataFormat::name),
            "nullable":self.nullable,"constraintNameRef":self.constraint_name_ref})
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelConfigDescriptor {
    pub schema_type_ref: String,
    pub config_parameters: Vec<ModelConfigParameter>,
}
impl ModelConfigDescriptor {
    pub fn to_json(&self) -> Result<String, ModelConfigBindingError> {
        self.validate()?;
        serde_json::to_string_pretty(&json!({"modelConfigDescriptor":{
            "schemaTypeRef":self.schema_type_ref,
            "configParameters":self.config_parameters.iter().map(ModelConfigParameter::value).collect::<Vec<_>>()
        }})).map_err(|e| failure(e.to_string()))
    }
    pub fn from_json(input: &str) -> Result<Self, ModelConfigBindingError> {
        let value: Value = serde_json::from_str(input).map_err(|e| failure(e.to_string()))?;
        let descriptor = &value["modelConfigDescriptor"];
        fn string(value: &Value, key: &str) -> Result<String, ModelConfigBindingError> {
            value[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| failure(format!("missing descriptor {key}")))
        }
        fn optional(value: &Value, key: &str) -> Result<Option<String>, ModelConfigBindingError> {
            if value[key].is_null() {
                Ok(None)
            } else {
                string(value, key).map(Some)
            }
        }
        let rows = descriptor["configParameters"]
            .as_array()
            .ok_or_else(|| failure("missing configuration parameters"))?;
        let mut config_parameters = Vec::with_capacity(rows.len());
        for row in rows {
            config_parameters.push(ModelConfigParameter {
                id: string(row, "id")?,
                name: string(row, "name")?,
                description: optional(row, "description")?,
                kind: match string(row, "kind")?.as_str() {
                    "WEIGHT" => ModelConfigParameterKind::Weight,
                    "PARAMETER" => ModelConfigParameterKind::Parameter,
                    _ => return Err(failure("unknown configuration parameter role")),
                },
                schema_type: ModelConfigSchemaType::parse(&string(row, "type")?)?,
                array_item_type: optional(row, "arrayItemType")?
                    .map(|v| ModelConfigSchemaType::parse(&v))
                    .transpose()?,
                schema_type_ref: optional(row, "schemaTypeRef")?,
                format: optional(row, "format")?
                    .map(|v| ModelConfigDataFormat::parse(&v))
                    .transpose()?,
                nullable: row["nullable"]
                    .as_bool()
                    .ok_or_else(|| failure("missing nullable flag"))?,
                constraint_name_ref: optional(row, "constraintNameRef")?,
            });
        }
        let result = Self {
            schema_type_ref: string(descriptor, "schemaTypeRef")?,
            config_parameters,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), ModelConfigBindingError> {
        nonempty(&self.schema_type_ref, "root schema reference")?;
        let mut ids = std::collections::BTreeSet::new();
        for p in &self.config_parameters {
            nonempty(&p.id, "parameter ID")?;
            nonempty(&p.name, "parameter name")?;
            if !ids.insert(&p.id) {
                return Err(failure(format!("duplicate parameter {:?}", p.id)));
            }
            if (p.schema_type == ModelConfigSchemaType::Array) != p.array_item_type.is_some() {
                return Err(failure("array parameter/item metadata mismatch"));
            }
            for reference in [&p.schema_type_ref, &p.constraint_name_ref]
                .into_iter()
                .flatten()
            {
                nonempty(reference, "parameter reference")?;
            }
            if p.format == Some(ModelConfigDataFormat::Duration)
                && p.schema_type != ModelConfigSchemaType::String
            {
                return Err(failure("duration metadata requires string type"));
            }
        }
        Ok(())
    }
    pub fn write_file(&self, path: impl AsRef<Path>) -> Result<(), ModelConfigBindingError> {
        std::fs::write(path, self.to_json()?).map_err(|e| failure(e.to_string()))
    }
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, ModelConfigBindingError> {
        Self::from_json(&std::fs::read_to_string(path).map_err(|e| failure(e.to_string()))?)
    }
}

type ReadMember<'a, T> = dyn Fn(&T) -> Result<Value, String> + 'a;
type ParseMember<'a> = dyn Fn(&Value) -> Result<Box<dyn Any>, String> + 'a;
type WriteMember<'a, T> = dyn Fn(&mut T, Box<dyn Any>) + 'a;
type ParsedMembers = Vec<(usize, Box<dyn Any>)>;
struct Member<'a, T> {
    member_name: String,
    parameter: ModelConfigParameter,
    shape: ModelConfigValueShape,
    read: Box<ReadMember<'a, T>>,
    parse: Box<ParseMember<'a>>,
    write: Box<WriteMember<'a, T>>,
}
/// Registration is ordered and explicit. Decoding validates all supplied values
/// before construction or mutation. Omitted values keep actual defaults/current
/// members. Input-error atomicity excludes caller callback effects/panics.
/// Duplicate JSON keys follow serde_json's last-value policy; no JVM scanning.
pub struct ModelConfigBinding<'a, T> {
    schema_type_ref: String,
    create: Box<dyn Fn() -> T + 'a>,
    members: Vec<Member<'a, T>>,
}
impl<'a, T> ModelConfigBinding<'a, T> {
    pub fn new(
        schema_type_ref: impl Into<String>,
        create: impl Fn() -> T + 'a,
    ) -> Result<Self, ModelConfigBindingError> {
        let schema_type_ref = schema_type_ref.into();
        nonempty(&schema_type_ref, "root schema reference")?;
        Ok(Self {
            schema_type_ref,
            create: Box::new(create),
            members: vec![],
        })
    }
    pub fn member<V: ModelConfigValue>(
        &mut self,
        member_name: impl Into<String>,
        options: ModelConfigMemberOptions,
        read: impl for<'t> Fn(&'t T) -> &'t V + 'a,
        write: impl Fn(&mut T, V) + 'a,
    ) -> Result<&mut Self, ModelConfigBindingError> {
        let member_name = member_name.into();
        nonempty(&member_name, "native member name")?;
        let mut shape = V::shape();
        if options.nullable == Some(true) && !shape.nullable {
            return Err(failure("nullable metadata requires optional typed storage"));
        }
        shape.nullable = options.nullable.unwrap_or(shape.nullable);
        shape.format = options.format.or(shape.format);
        shape.validate()?;
        let id = options.id.unwrap_or_else(|| member_name.clone());
        let parameter = ModelConfigParameter {
            name: options.name.unwrap_or_else(|| id.clone()),
            id,
            description: options.description,
            kind: options
                .kind
                .unwrap_or(if options.constraint_name_ref.is_some() {
                    ModelConfigParameterKind::Weight
                } else {
                    ModelConfigParameterKind::Parameter
                }),
            schema_type: shape.schema_type,
            array_item_type: shape.items.as_ref().map(|s| s.schema_type),
            schema_type_ref: shape.reference().map(str::to_owned),
            format: shape.format,
            nullable: shape.nullable,
            constraint_name_ref: options.constraint_name_ref,
        };
        let mut descriptor = self.descriptor();
        descriptor.config_parameters.push(parameter.clone());
        descriptor.validate()?;
        if self.members.iter().any(|m| m.member_name == member_name) {
            return Err(failure(format!("duplicate native member {member_name:?}")));
        }
        // Validate named definitions before publishing a member or calling code.
        let mut definitions = BTreeMap::new();
        for m in &self.members {
            m.shape.schema(&mut definitions)?;
        }
        shape.schema(&mut definitions)?;
        if definitions.contains_key(&self.schema_type_ref) {
            return Err(failure(
                "object schema reference collides with configuration root",
            ));
        }
        self.members.push(Member {
            member_name,
            parameter,
            shape,
            read: Box::new(move |t| read(t).to_config_json()),
            parse: Box::new(|value| {
                V::from_config_json(value).map(|v| Box::new(v) as Box<dyn Any>)
            }),
            write: Box::new(move |t, value| {
                write(
                    t,
                    *value
                        .downcast::<V>()
                        .expect("typed member decoder invariant"),
                )
            }),
        });
        Ok(self)
    }
    pub fn descriptor(&self) -> ModelConfigDescriptor {
        ModelConfigDescriptor {
            schema_type_ref: self.schema_type_ref.clone(),
            config_parameters: self.members.iter().map(|m| m.parameter.clone()).collect(),
        }
    }
    pub fn schema_json(&self) -> Result<String, ModelConfigBindingError> {
        let mut definitions = BTreeMap::new();
        let mut properties = Map::new();
        for member in &self.members {
            let mut value = member.shape.schema(&mut definitions)?;
            value["title"] = json!(member.parameter.name);
            if let Some(description) = &member.parameter.description {
                value["description"] = json!(description);
            }
            properties.insert(member.parameter.id.clone(), value);
        }
        definitions.insert(
            self.schema_type_ref.clone(),
            json!({"type":"object", "properties":properties, "additionalProperties":false}),
        );
        serde_json::to_string_pretty(&json!({"$schema":"https://json-schema.org/draft/2020-12/schema",
            "$ref":format!("#/$defs/{}", pointer_segment(&self.schema_type_ref)), "$defs":definitions}))
            .map_err(|e| failure(e.to_string()))
    }
    pub fn write_schema(&self, path: impl AsRef<Path>) -> Result<(), ModelConfigBindingError> {
        std::fs::write(path, self.schema_json()?).map_err(|e| failure(e.to_string()))
    }
    fn parse(&self, input: &str) -> Result<ParsedMembers, ModelConfigBindingError> {
        let value: Value = serde_json::from_str(input).map_err(|e| failure(e.to_string()))?;
        check_numeric_lexemes(input)?;
        let object = value
            .as_object()
            .ok_or_else(|| failure("configuration must be an object"))?;
        for key in object.keys() {
            if !self.members.iter().any(|m| &m.parameter.id == key) {
                return Err(failure(format!("unknown configuration member {key:?}")));
            }
        }
        let mut parsed = vec![];
        for (i, member) in self.members.iter().enumerate() {
            if let Some(value) = object.get(&member.parameter.id) {
                if value.is_null() && !member.parameter.nullable {
                    return Err(failure(format!(
                        "member {:?} is not nullable",
                        member.parameter.id
                    )));
                }
                let v = (member.parse)(value)
                    .map_err(|e| failure(format!("member {:?}: {e}", member.parameter.id)))?;
                parsed.push((i, v));
            }
        }
        Ok(parsed)
    }
    pub fn decode(&self, input: &str) -> Result<T, ModelConfigBindingError> {
        let parsed = self.parse(input)?;
        let mut record = (self.create)();
        self.install(&mut record, parsed);
        Ok(record)
    }
    pub fn apply_json(&self, record: &mut T, input: &str) -> Result<(), ModelConfigBindingError> {
        let parsed = self.parse(input)?;
        self.install(record, parsed);
        Ok(())
    }
    fn install(&self, record: &mut T, parsed: ParsedMembers) {
        for (i, value) in parsed {
            (self.members[i].write)(record, value);
        }
    }
    fn read(&self, member: &Member<'a, T>, record: &T) -> Result<Value, ModelConfigBindingError> {
        let value = (member.read)(record)
            .map_err(|e| failure(format!("member {:?}: {e}", member.parameter.id)))?;
        if value.is_null() && !member.parameter.nullable {
            return Err(failure(format!(
                "member {:?} is not nullable",
                member.parameter.id
            )));
        }
        (member.parse)(&value)
            .map_err(|e| failure(format!("member {:?}: {e}", member.parameter.id)))?;
        Ok(value)
    }
    pub fn encode(&self, record: &T) -> Result<String, ModelConfigBindingError> {
        let mut object = Map::new();
        for member in &self.members {
            object.insert(member.parameter.id.clone(), self.read(member, record)?);
        }
        serde_json::to_string(&object).map_err(|e| failure(e.to_string()))
    }
    /// Select actual weight members explicitly. All IDs, roles and unique target
    /// references validate before getters or conversion callbacks run. Repeated
    /// descriptive references are legal metadata, but ambiguous runtime selections
    /// are rejected. Caller conversion supplies the native score type/levels.
    pub fn constraint_weights<S>(
        &self,
        record: &T,
        ids: &[&str],
        score: impl Fn(&ModelConfigParameter, &Value) -> Result<S, String>,
    ) -> Result<ConstraintWeightOverrides<S>, ModelConfigBindingError> {
        let mut selected = vec![];
        let mut targets = std::collections::BTreeSet::new();
        for id in ids {
            let member = self
                .members
                .iter()
                .find(|m| &m.parameter.id == id)
                .ok_or_else(|| failure(format!("unknown weight member {id:?}")))?;
            if member.parameter.kind != ModelConfigParameterKind::Weight {
                return Err(failure(format!("member {id:?} is not a weight")));
            }
            let name = member
                .parameter
                .constraint_name_ref
                .as_deref()
                .unwrap_or(id);
            if !targets.insert(name) {
                return Err(failure(format!("duplicate weight target {name:?}")));
            }
            selected.push((name, member));
        }
        let mut weights = ConstraintWeightOverrides::default();
        for (name, member) in selected {
            let value = self.read(member, record)?;
            weights.insert(name, score(&member.parameter, &value).map_err(failure)?);
        }
        Ok(weights)
    }
}

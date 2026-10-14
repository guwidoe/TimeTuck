// Adapted Community TEST helpers; Apache-2.0. See model_config/NOTICE.txt.
#![allow(dead_code)]
use serde_json::{Value, json};
use timetuck::*;

pub const CONFIG_TYPE: &str = "TestdataModelConfigOverrides";
pub const CONFLICTS: &str = "noConflicts";
pub const DEFAULT_WEIGHT_ONE: i64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectParameter {
    pub id: i64,
    pub value: String,
}
impl ModelConfigValue for ObjectParameter {
    fn shape() -> ModelConfigValueShape {
        ModelConfigValueShape::object(
            "ObjectParameter",
            [
                ("id".into(), i64::shape()),
                ("value".into(), String::shape()),
            ]
            .into_iter()
            .collect(),
        )
    }
    fn from_config_json(value: &Value) -> Result<Self, String> {
        let o = value.as_object().ok_or("expected object parameter")?;
        if o.len() != 2 || !o.contains_key("id") || !o.contains_key("value") {
            return Err("object parameter requires exactly id and value".into());
        }
        Ok(Self {
            id: i64::from_config_json(&o["id"])?,
            value: String::from_config_json(&o["value"])?,
        })
    }
    fn to_config_json(&self) -> Result<Value, String> {
        Ok(json!({"id":self.id,"value":self.value}))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultOverrides {
    pub constraint_weight: i64,
    pub primitive: i64,
    pub primitive_array: Vec<i32>,
    pub duration: JsonDuration,
    pub string: Option<String>,
    pub string_list: Vec<String>,
    pub string_array: Vec<String>,
    pub object: Option<ObjectParameter>,
    pub object_list: Vec<ObjectParameter>,
    pub object_array: Vec<ObjectParameter>,
}
impl Default for DefaultOverrides {
    fn default() -> Self {
        Self {
            constraint_weight: DEFAULT_WEIGHT_ONE,
            primitive: DEFAULT_WEIGHT_ONE,
            primitive_array: vec![],
            duration: "PT0S".parse().unwrap(),
            string: None,
            string_list: vec![],
            string_array: vec![],
            object: None,
            object_list: vec![],
            object_array: vec![],
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaOverrides {
    pub constraint_weight: i64,
    pub primitive: i64,
    pub primitive_array: Vec<i32>,
    pub duration: JsonDuration,
    pub string: Option<String>,
    pub big_decimal_constraint_weight: Decimal,
}
impl Default for SchemaOverrides {
    fn default() -> Self {
        Self {
            constraint_weight: DEFAULT_WEIGHT_ONE,
            primitive: DEFAULT_WEIGHT_ONE,
            primitive_array: vec![],
            duration: "PT0S".parse().unwrap(),
            string: None,
            big_decimal_constraint_weight: Decimal::ZERO,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceOverrides {
    pub constraint_weight_by_default: i64,
    pub parameter: i64,
    pub constraint_weight_by_annotation: i64,
}
impl Default for ReferenceOverrides {
    fn default() -> Self {
        Self {
            constraint_weight_by_default: DEFAULT_WEIGHT_ONE,
            parameter: 10,
            constraint_weight_by_annotation: DEFAULT_WEIGHT_ONE,
        }
    }
}
macro_rules! member {
    ($b:expr, $t:ty, $name:expr, $field:ident, $options:expr) => {
        $b.member($name, $options, |t: &$t| &t.$field, |t, v| t.$field = v)
            .unwrap();
    };
}
fn weight() -> ModelConfigMemberOptions {
    ModelConfigMemberOptions {
        kind: Some(ModelConfigParameterKind::Weight),
        ..Default::default()
    }
}
pub fn default_binding() -> ModelConfigBinding<'static, DefaultOverrides> {
    let mut b = ModelConfigBinding::new(CONFIG_TYPE, DefaultOverrides::default).unwrap();
    member!(
        b,
        DefaultOverrides,
        "constraintWeight",
        constraint_weight,
        weight()
    );
    member!(
        b,
        DefaultOverrides,
        "primitive",
        primitive,
        Default::default()
    );
    member!(
        b,
        DefaultOverrides,
        "primitiveArray",
        primitive_array,
        Default::default()
    );
    member!(
        b,
        DefaultOverrides,
        "duration",
        duration,
        Default::default()
    );
    member!(b, DefaultOverrides, "string", string, Default::default());
    member!(
        b,
        DefaultOverrides,
        "stringList",
        string_list,
        Default::default()
    );
    member!(
        b,
        DefaultOverrides,
        "stringArray",
        string_array,
        Default::default()
    );
    member!(b, DefaultOverrides, "object", object, Default::default());
    member!(
        b,
        DefaultOverrides,
        "objectList",
        object_list,
        Default::default()
    );
    member!(
        b,
        DefaultOverrides,
        "objectArray",
        object_array,
        Default::default()
    );
    b
}
pub fn schema_binding() -> ModelConfigBinding<'static, SchemaOverrides> {
    let mut b = ModelConfigBinding::new(CONFIG_TYPE, SchemaOverrides::default).unwrap();
    member!(
        b,
        SchemaOverrides,
        "constraintWeight",
        constraint_weight,
        ModelConfigMemberOptions {
            name: Some("Constraint weight".into()),
            description: Some("Constraint weight description".into()),
            ..weight()
        }
    );
    member!(
        b,
        SchemaOverrides,
        "primitive",
        primitive,
        ModelConfigMemberOptions {
            id: Some("primitiveParam".into()),
            format: Some(ModelConfigDataFormat::Percentage),
            ..Default::default()
        }
    );
    member!(
        b,
        SchemaOverrides,
        "primitiveArray",
        primitive_array,
        ModelConfigMemberOptions {
            id: Some("primitiveArrayParam".into()),
            name: Some("Array of numbers".into()),
            format: Some(ModelConfigDataFormat::Number),
            ..Default::default()
        }
    );
    member!(
        b,
        SchemaOverrides,
        "duration",
        duration,
        ModelConfigMemberOptions {
            name: Some("Duration".into()),
            format: Some(ModelConfigDataFormat::Duration),
            ..Default::default()
        }
    );
    member!(
        b,
        SchemaOverrides,
        "string",
        string,
        ModelConfigMemberOptions {
            description: Some("String parameter".into()),
            nullable: Some(true),
            ..Default::default()
        }
    );
    member!(
        b,
        SchemaOverrides,
        "bigDecimalConstraintWeight",
        big_decimal_constraint_weight,
        weight()
    );
    b
}
pub fn reference_binding() -> ModelConfigBinding<'static, ReferenceOverrides> {
    let mut b = ModelConfigBinding::new(CONFIG_TYPE, ReferenceOverrides::default).unwrap();
    let reference = ModelConfigMemberOptions {
        constraint_name_ref: Some(CONFLICTS.into()),
        ..Default::default()
    };
    member!(
        b,
        ReferenceOverrides,
        "constraintWeightByDefault",
        constraint_weight_by_default,
        reference.clone()
    );
    member!(
        b,
        ReferenceOverrides,
        "parameter",
        parameter,
        ModelConfigMemberOptions {
            kind: Some(ModelConfigParameterKind::Parameter),
            ..reference.clone()
        }
    );
    member!(
        b,
        ReferenceOverrides,
        "constraintWeightByAnnotation",
        constraint_weight_by_annotation,
        ModelConfigMemberOptions {
            kind: Some(ModelConfigParameterKind::Weight),
            ..reference
        }
    );
    b
}

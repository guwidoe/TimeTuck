// Complete Community metadata assertions; Apache-2.0. Native registrations replace deployment scanning.
#[path = "fixtures/model_config_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use timetuck::*;

fn generated<T>(binding: &ModelConfigBinding<'_, T>) -> ModelConfigDescriptor {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "timetuck-model-config-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let descriptor = dir.join("timefold-model-descriptor.json");
    let schema = dir.join("model-config-schema.json");
    binding.descriptor().write_file(&descriptor).unwrap();
    binding.write_schema(&schema).unwrap();
    let loaded = ModelConfigDescriptor::read_file(&descriptor).unwrap();
    assert_eq!(loaded, binding.descriptor());
    let actual: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(schema).unwrap()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(&binding.schema_json().unwrap()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual["$ref"], format!("#/$defs/{CONFIG_TYPE}"));
    assert_eq!(
        actual["$defs"][CONFIG_TYPE]["properties"]
            .as_object()
            .unwrap()
            .len(),
        loaded.config_parameters.len()
    );
    std::fs::remove_dir_all(dir).unwrap();
    loaded
}
#[test]
fn detect_default_model_config_overrides() {
    let d = generated(&default_binding());
    assert_eq!(d.schema_type_ref, CONFIG_TYPE);
    assert_eq!(d.config_parameters.len(), 10);
    use ModelConfigSchemaType::{Array, Integer, Object, String as Text};
    let rows = [
        (
            "constraintWeight",
            ModelConfigParameterKind::Weight,
            Integer,
        ),
        ("primitive", ModelConfigParameterKind::Parameter, Integer),
        ("primitiveArray", ModelConfigParameterKind::Parameter, Array),
        ("duration", ModelConfigParameterKind::Parameter, Text),
        ("string", ModelConfigParameterKind::Parameter, Text),
        ("stringList", ModelConfigParameterKind::Parameter, Array),
        ("stringArray", ModelConfigParameterKind::Parameter, Array),
        ("object", ModelConfigParameterKind::Parameter, Object),
        ("objectList", ModelConfigParameterKind::Parameter, Array),
        ("objectArray", ModelConfigParameterKind::Parameter, Array),
    ];
    for (p, (id, kind, ty)) in d.config_parameters.iter().zip(rows) {
        assert_eq!(p.id, id);
        assert_eq!(p.name, id);
        assert_eq!(p.description, None);
        assert_eq!(p.kind, kind);
        assert_eq!(p.schema_type, ty);
    }
    let p = &d.config_parameters;
    assert_eq!(p[2].array_item_type, Some(Integer));
    assert_eq!(p[2].schema_type_ref, None);
    assert_eq!(p[3].format, Some(ModelConfigDataFormat::Duration));
    assert_eq!(p[3].schema_type_ref.as_deref(), Some("Duration"));
    assert_eq!(p[4].schema_type_ref, None);
    for i in [5, 6] {
        assert_eq!(p[i].array_item_type, Some(Text));
        assert_eq!(p[i].schema_type_ref, None);
    }
    assert_eq!(p[7].schema_type_ref.as_deref(), Some("ObjectParameter"));
    for i in [8, 9] {
        assert_eq!(p[i].array_item_type, Some(Object));
        assert_eq!(p[i].schema_type_ref.as_deref(), Some("ObjectParameter"));
    }
}
#[test]
fn detect_schema_model_config_overrides() {
    let d = generated(&schema_binding());
    assert_eq!(d.schema_type_ref, CONFIG_TYPE);
    assert_eq!(d.config_parameters.len(), 6);
    let p = &d.config_parameters;
    assert_eq!(p[0].id, "constraintWeight");
    assert_eq!(p[0].name, "Constraint weight");
    assert_eq!(
        p[0].description.as_deref(),
        Some("Constraint weight description")
    );
    assert_eq!(p[0].kind, ModelConfigParameterKind::Weight);
    assert_eq!(p[0].schema_type, ModelConfigSchemaType::Integer);
    assert_eq!(p[1].id, "primitiveParam");
    assert_eq!(p[1].name, "primitiveParam");
    assert_eq!(p[1].description, None);
    assert_eq!(p[1].format, Some(ModelConfigDataFormat::Percentage));
    assert_eq!(p[1].kind, ModelConfigParameterKind::Parameter);
    assert_eq!(p[1].schema_type, ModelConfigSchemaType::Integer);
    assert!(!p[1].nullable);
    assert_eq!(p[2].id, "primitiveArrayParam");
    assert_eq!(p[2].name, "Array of numbers");
    assert_eq!(p[2].description, None);
    assert_eq!(p[2].format, Some(ModelConfigDataFormat::Number));
    assert_eq!(p[2].kind, ModelConfigParameterKind::Parameter);
    assert_eq!(p[2].schema_type, ModelConfigSchemaType::Array);
    assert_eq!(p[2].schema_type_ref, None);
    assert_eq!(p[3].id, "duration");
    assert_eq!(p[3].name, "Duration");
    assert_eq!(p[3].description, None);
    assert_eq!(p[3].format, Some(ModelConfigDataFormat::Duration));
    assert_eq!(p[3].kind, ModelConfigParameterKind::Parameter);
    assert_eq!(p[3].schema_type, ModelConfigSchemaType::String);
    assert_eq!(p[3].schema_type_ref.as_deref(), Some("Duration"));
    assert_eq!(p[4].id, "string");
    assert_eq!(p[4].name, "string");
    assert_eq!(p[4].description.as_deref(), Some("String parameter"));
    assert_eq!(p[4].kind, ModelConfigParameterKind::Parameter);
    assert_eq!(p[4].schema_type, ModelConfigSchemaType::String);
    assert_eq!(p[4].schema_type_ref, None);
    assert!(p[4].nullable);
    assert_eq!(p[5].id, "bigDecimalConstraintWeight");
    assert_eq!(p[5].kind, ModelConfigParameterKind::Weight);
    assert_eq!(p[5].schema_type, ModelConfigSchemaType::Number);
    assert_eq!(p[5].schema_type_ref, None);
}
#[test]
fn detect_constraint_reference_model_config_overrides() {
    let d = generated(&reference_binding());
    assert_eq!(d.config_parameters.len(), 3);
    for (p, (id, kind)) in d.config_parameters.iter().zip([
        (
            "constraintWeightByDefault",
            ModelConfigParameterKind::Weight,
        ),
        ("parameter", ModelConfigParameterKind::Parameter),
        (
            "constraintWeightByAnnotation",
            ModelConfigParameterKind::Weight,
        ),
    ]) {
        assert_eq!(p.id, id);
        assert_eq!(p.kind, kind);
        assert_eq!(p.constraint_name_ref.as_deref(), Some(CONFLICTS));
    }
}

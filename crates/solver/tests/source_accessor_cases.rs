// Adapted from pinned Community RootVariableSourceTest. Apache-2.0; see provenance.
#[path = "fixtures/source_accessor_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn invalid_parameter() {
    let schema = invalid_schema();
    let registry = SourcePathRegistry::new(&schema);
    let result=registry.solution_supplier::<TestdataInvalidDeclarativeParameterValue,i32,TestdataInvalidDeclarativeParameterValue>("invalidParameter",|_,_|panic!("invalid supplier must never run"));
    let error = result.err().unwrap();
    let expected = format!(
        "Maybe you included a parameter which is not a planning solution ({})?",
        PlanningType::of::<TestdataInvalidDeclarativeParameterSolution>().name()
    );
    assert!(error.to_string().contains(&expected));
}
#[test]
fn prefer_getter_when_field_the_same_type() {
    struct TestClass {
        name: String,
    }
    let object = TestClass {
        name: "name".into(),
    };
    let selected = SourceAccessorOptions::default()
        .field(|v: &TestClass| Some(&v.name))
        .getter(|v: &TestClass| Some(&v.name))
        .select::<TestClass>("name", "name", &SourceTypeHierarchy::default())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Getter);
    assert_eq!(
        selected.read(&object).unwrap().downcast_ref::<String>(),
        Some(&object.name)
    );
}
#[test]
fn prefer_getter_when_getter_is_more_specific_than_field() {
    struct TestClass {
        value: SubValue,
    }
    let object = TestClass {
        value: SubValue::sample(),
    };
    let selected = SourceAccessorOptions::default()
        .field(|v: &TestClass| Some(&v.value.base))
        .getter(|v: &TestClass| Some(&v.value))
        .select::<TestClass>("value", "value", &hierarchy())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Getter);
    assert!(std::ptr::eq(
        selected
            .read(&object)
            .unwrap()
            .downcast_ref::<SubValue>()
            .unwrap(),
        &object.value
    ));
}
#[test]
fn prefer_getter_when_getter_is_not_covariant_with_field() {
    struct TestClass {
        value: String,
    }
    static ONE: i32 = 1;
    let object = TestClass {
        value: "value".into(),
    };
    let selected = SourceAccessorOptions::default()
        .field(|v: &TestClass| Some(&v.value))
        .getter(|_: &TestClass| Some(&ONE))
        .select::<TestClass>("value", "value", &SourceTypeHierarchy::default())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Getter);
    assert_eq!(
        selected.read(&object).unwrap().downcast_ref::<i32>(),
        Some(&1)
    );
}
#[test]
fn prefer_field_when_field_more_specific_than_getter() {
    struct TestClass {
        value: SubValue,
    }
    let object = TestClass {
        value: SubValue::sample(),
    };
    let selected = SourceAccessorOptions::default()
        .field(|v: &TestClass| Some(&v.value))
        .getter(|v: &TestClass| Some(&v.value.base))
        .select::<TestClass>("value", "value", &hierarchy())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Field);
    assert!(std::ptr::eq(
        selected
            .read(&object)
            .unwrap()
            .downcast_ref::<SubValue>()
            .unwrap(),
        &object.value
    ));
}
#[test]
fn use_field_if_no_getter() {
    struct TestClass {
        value: String,
    }
    let object = TestClass {
        value: "value".into(),
    };
    let selected = SourceAccessorOptions::default()
        .field(|v: &TestClass| Some(&v.value))
        .select::<TestClass>("value", "value", &SourceTypeHierarchy::default())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Field);
    assert_eq!(
        selected.read(&object).unwrap().downcast_ref::<String>(),
        Some(&object.value)
    );
}
#[test]
fn use_getter_if_no_field() {
    struct TestClass;
    static VALUE: &str = "value";
    let selected = SourceAccessorOptions::default()
        .getter(|_: &TestClass| Some(&VALUE))
        .select::<TestClass>("value", "value", &SourceTypeHierarchy::default())
        .unwrap();
    assert_eq!(selected.kind(), SourceAccessorKind::Getter);
    assert_eq!(
        selected.read(&TestClass).unwrap().downcast_ref::<&str>(),
        Some(&"value")
    );
}
#[test]
fn error_if_no_member() {
    struct TestClass;
    struct RootClass {
        inner: TestClass,
    }
    let root = RootClass { inner: TestClass };
    let _ = &root.inner;
    let error = SourceAccessorOptions::<TestClass>::default()
        .select::<RootClass>("inner.value", "value", &SourceTypeHierarchy::default())
        .err()
        .unwrap();
    let text = error.to_string();
    for fragment in [
        "The source path (inner.value)",
        "starting from root class (RootClass)",
        "references a member (value)",
        "on class (TestClass)",
        "that does not exist.",
    ] {
        assert!(text.contains(fragment), "{text}");
    }
}

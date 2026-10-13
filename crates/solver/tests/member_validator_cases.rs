// Adapted pinned Community MemberAccessorValidatorTest; Apache-2.0.
use timetuck::*;
include!("fixtures/member_validator_fixture.rs");
#[test]
fn members_in_private_class_fail() {
    for name in ["publicField", "privateFieldWithGetter"] {
        let member = private_field(name);
        let error = member
            .bind(MemberAccessRole::FieldOrGetter, None)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(&format!("The field ({name}) in class ({PRIVATE})")));
        assert!(error.contains(&format!("its declaring class ({PRIVATE}) is not public")));
    }
    let member = private_method("publicVoidMethod");
    let error = member
        .bind(MemberAccessRole::Void, None)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains(&format!(
        "The method (publicVoidMethod) in class ({PRIVATE})"
    )));
    assert!(error.contains(&format!("its declaring class ({PRIVATE}) is not public")));
}
#[test]
fn field_or_getter_method() {
    {
        let member = field("publicField");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "publicField");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "privateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "privateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "privateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "getPrivateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithoutGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithoutGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is not public and does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The method (getVoidMethod) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"has a public getter method that returns void".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is a getter method, but it is not public".to_string()),
            "{error}"
        );
    }
    {
        let member = method("publicReadMethod");
        let result = member.bind(MemberAccessRole::FieldOrGetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (publicReadMethod) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(error.contains(&"is suppose to be a public getter method, but its name (publicReadMethod) does not start with \"get\"".to_string()),"{error}");
    }
}
#[test]
fn field_or_getter_method_with_setter() {
    {
        let member = field("publicField");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        assert!(result.is_ok(), "publicField");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        assert!(result.is_ok(), "privateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        assert!(result.is_ok(), "privateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a setter method and is not public".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithoutGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithoutGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is not public and does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithPrivateSetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a setter method and is not public".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithPrivateSetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(
                &"requires both a public getter and a public setter but only have a public getter"
                    .to_string()
            ),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(
                &"requires both a public getter and a public setter but only have a public getter"
                    .to_string()
            ),
            "{error}"
        );
    }
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The method (getVoidMethod) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"has a public getter method that returns void".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The method (getVoidMethod) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"has a public getter method that returns void".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is a getter method, but it is not public".to_string()),
            "{error}"
        );
    }
    {
        let member = method("publicReadMethod");
        let result = member.bind(MemberAccessRole::FieldOrGetterWithSetter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (publicReadMethod) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(error.contains(&"is suppose to be a public getter method, but its name (publicReadMethod) does not start with \"get\"".to_string()),"{error}");
    }
}
#[test]
fn field_or_read_method() {
    {
        let member = field("publicField");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "publicField");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "privateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "privateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "privateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "getPrivateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("publicReadMethod");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        assert!(result.is_ok(), "publicReadMethod");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithoutGetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithoutGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is not public and does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The method (getVoidMethod) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"is a public read method, but it returns void".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is a read method, but it is not public.".to_string()),
            "{error}"
        );
    }
    {
        let member = method("publicReadMethodWithParameter");
        let result = member.bind(MemberAccessRole::FieldOrRead, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (publicReadMethodWithParameter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(
                &"is a public read method, but takes (1) parameters instead of none".to_string()
            ),
            "{error}"
        );
    }
}
#[test]
fn field_or_read_method_with_optional_parameter() {
    {
        let member = field("publicField");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "publicField");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "privateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "privateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "privateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithPrivateSetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "getPrivateFieldWithPrivateSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("getPrivateFieldWithGetterAndSetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "getPrivateFieldWithGetterAndSetter");
        execute_valid(result.unwrap());
    }
    {
        let member = method("publicReadMethod");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "publicReadMethod");
        execute_valid(result.unwrap());
    }
    {
        let member = method("publicReadMethodWithParameter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        assert!(result.is_ok(), "publicReadMethodWithParameter");
        execute_valid(result.unwrap());
    }
    {
        let member = field("privateFieldWithoutGetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithoutGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is not public and does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"does not have a public getter method".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The method (getVoidMethod) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"is a public read method, but it returns void".to_string()),
            "{error}"
        );
    }
    {
        let member = method("getPrivateFieldWithPrivateGetter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (getPrivateFieldWithPrivateGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is a read method, but it is not public.".to_string()),
            "{error}"
        );
    }
    {
        let member = method("publicReadMethodWithManyParameter");
        let result = member.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (publicReadMethodWithManyParameter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(
                &"is a public read method, but takes (2) parameters instead of zero or one"
                    .to_string()
            ),
            "{error}"
        );
    }
}
#[test]
fn void_method() {
    {
        let member = method("getVoidMethod");
        let result = member.bind(MemberAccessRole::Void, None);
        assert!(result.is_ok(), "getVoidMethod");
        execute_valid(result.unwrap());
    }
    {
        let member = field("publicField");
        let result = member.bind(MemberAccessRole::Void, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!("The field (publicField) in class ({})", PUBLIC)),
            "{error}"
        );
        assert!(
            error.contains(&"must be a void method, but is a field instead".to_string()),
            "{error}"
        );
    }
    {
        let member = field("privateFieldWithGetter");
        let result = member.bind(MemberAccessRole::Void, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The field (privateFieldWithGetter) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"must be a void method, but is a field instead".to_string()),
            "{error}"
        );
    }
    {
        let member = method("publicReadMethod");
        let result = member.bind(MemberAccessRole::Void, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (publicReadMethod) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"must be a void method, but it returns (java.lang.String)".to_string()),
            "{error}"
        );
    }
    {
        let member = method("privateVoidMethod");
        let result = member.bind(MemberAccessRole::Void, None);
        let error = result.err().expect("invalid member").to_string();
        assert!(
            error.contains(&format!(
                "The method (privateVoidMethod) in class ({})",
                PUBLIC
            )),
            "{error}"
        );
        assert!(
            error.contains(&"is a void method, but it is not public".to_string()),
            "{error}"
        );
    }
}
#[test]
fn include_annotation_info_if_present() {
    for (member, fragment) in [
        (
            field("privateFieldWithoutGetter"),
            "@PlanningId annotated field",
        ),
        (
            method("getPrivateFieldWithPrivateGetter"),
            "@PlanningId annotated method",
        ),
    ] {
        let error = member
            .bind(MemberAccessRole::FieldOrGetter, Some("PlanningId"))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(fragment));
        assert!(error.contains(if fragment.ends_with("field") {
            "privateFieldWithoutGetter"
        } else {
            "getPrivateFieldWithPrivateGetter"
        }));
    }
}

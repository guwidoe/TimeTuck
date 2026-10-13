// Adapted PublicClass/PrivateClass and assertion helpers from the pinned TEST;
// Apache-2.0, see compat/provenance.json. Explicit labels replace Java reflection.
const PUBLIC: &str =
    "ai.timefold.solver.core.impl.domain.common.accessor.MemberAccessorValidatorTest.PublicClass";
const PRIVATE: &str =
    "ai.timefold.solver.core.impl.domain.common.accessor.MemberAccessorValidatorTest.PrivateClass";
#[derive(Default)]
struct PublicClass {
    public_field: Option<String>,
    without_getter: Option<String>,
    with_getter: Option<String>,
    with_both: Option<String>,
    with_private_getter: Option<String>,
    with_private_setter: Option<String>,
    calls: usize,
}
type Member = RegisteredMember<PublicClass, Option<String>, Option<String>>;
fn read(name: &str) -> fn(&PublicClass) -> Option<String> {
    match name {
        "publicField" => |e| e.public_field.clone(),
        "privateFieldWithoutGetter" => |e| e.without_getter.clone(),
        "privateFieldWithGetter" => |e| e.with_getter.clone(),
        "privateFieldWithGetterAndSetter" => |e| e.with_both.clone(),
        "privateFieldWithPrivateGetter" => |e| e.with_private_getter.clone(),
        "privateFieldWithPrivateSetter" => |e| e.with_private_setter.clone(),
        _ => panic!("unknown actual field"),
    }
}
fn write(name: &str) -> fn(&mut PublicClass, Option<String>) {
    match name {
        "publicField" => |e, v| e.public_field = v,
        "privateFieldWithoutGetter" => |e, v| e.without_getter = v,
        "privateFieldWithGetter" => |e, v| e.with_getter = v,
        "privateFieldWithGetterAndSetter" => |e, v| e.with_both = v,
        "privateFieldWithPrivateGetter" => |e, v| e.with_private_getter = v,
        "privateFieldWithPrivateSetter" => |e, v| e.with_private_setter = v,
        _ => panic!("unknown actual field"),
    }
}
fn field(name: &str) -> Member {
    use MemberExposure::*;
    let getter = match name {
        "publicField" | "privateFieldWithoutGetter" => None,
        "privateFieldWithPrivateGetter" => Some((
            Internal,
            Box::new(read(name)) as Box<dyn Fn(&PublicClass) -> Option<String>>,
        )),
        _ => Some((
            Public,
            Box::new(read(name)) as Box<dyn Fn(&PublicClass) -> Option<String>>,
        )),
    };
    let setter = match name {
        "privateFieldWithGetterAndSetter" => Some((
            Public,
            Box::new(write(name)) as Box<dyn Fn(&mut PublicClass, Option<String>)>,
        )),
        "privateFieldWithPrivateSetter" => Some((
            Internal,
            Box::new(write(name)) as Box<dyn Fn(&mut PublicClass, Option<String>)>,
        )),
        _ => None,
    };
    Member::new(
        PUBLIC,
        Public,
        name,
        if name == "publicField" {
            Public
        } else {
            Internal
        },
        "java.lang.String",
        RegisteredMemberBody::Field {
            read: Box::new(read(name)),
            write: Some(Box::new(write(name))),
            getter,
            setter,
        },
    )
}
fn method(name: &str) -> Member {
    use MemberExposure::*;
    let exposure = if matches!(
        name,
        "getPrivateFieldWithPrivateGetter" | "privateVoidMethod" | "privateReadMethod"
    ) {
        Internal
    } else {
        Public
    };
    let body = match name {
        "getVoidMethod" | "privateVoidMethod" => {
            RegisteredMemberBody::Void(Box::new(|e: &mut PublicClass| e.calls += 1))
        }
        "publicReadMethod" | "privateReadMethod" => {
            RegisteredMemberBody::Read(Box::new(|_: &PublicClass| None))
        }
        "publicReadMethodWithParameter" => {
            RegisteredMemberBody::ReadOne(Box::new(|_: &PublicClass, p| p))
        }
        "publicReadMethodWithManyParameter" => {
            RegisteredMemberBody::ReadTwo(Box::new(|_: &PublicClass, p, _| p))
        }
        _ => {
            let n = name.strip_prefix("get").unwrap();
            let mut chars = n.chars();
            let n = chars.next().unwrap().to_lowercase().to_string() + chars.as_str();
            RegisteredMemberBody::Read(Box::new(read(&n)))
        }
    };
    let member = Member::new(PUBLIC, Public, name, exposure, "java.lang.String", body);
    match name {
        "getPrivateFieldWithGetterAndSetter" => {
            member.with_setter(Public, write("privateFieldWithGetterAndSetter"))
        }
        "getPrivateFieldWithPrivateSetter" => {
            member.with_setter(Internal, write("privateFieldWithPrivateSetter"))
        }
        _ => member,
    }
}
#[derive(Default)]
struct PrivateClass {
    public_field: Option<String>,
    with_getter: Option<String>,
    calls: usize,
}
fn private_field(name: &str) -> RegisteredMember<PrivateClass, Option<String>, Option<String>> {
    use MemberExposure::*;
    RegisteredMember::new(
        PRIVATE,
        Internal,
        name,
        if name == "publicField" {
            Public
        } else {
            Internal
        },
        "java.lang.String",
        RegisteredMemberBody::Field {
            read: if name == "publicField" {
                Box::new(|e: &PrivateClass| e.public_field.clone())
            } else {
                Box::new(|e: &PrivateClass| e.with_getter.clone())
            },
            write: if name == "publicField" {
                Some(Box::new(|e: &mut PrivateClass, v| e.public_field = v))
            } else {
                Some(Box::new(|e: &mut PrivateClass, v| e.with_getter = v))
            },
            getter: if name == "publicField" {
                None
            } else {
                Some((Public, Box::new(|e: &PrivateClass| e.with_getter.clone())))
            },
            setter: None,
        },
    )
}
fn private_method(name: &str) -> RegisteredMember<PrivateClass, Option<String>, Option<String>> {
    RegisteredMember::new(
        PRIVATE,
        MemberExposure::Internal,
        name,
        MemberExposure::Public,
        "void",
        RegisteredMemberBody::Void(Box::new(|e: &mut PrivateClass| e.calls += 1)),
    )
}
fn execute_valid(binding: BoundRegisteredMember<'_, PublicClass, Option<String>, Option<String>>) {
    let mut entity = PublicClass::default();
    match binding.read(&entity, Some(Some("parameter".into()))) {
        Ok(value) => {
            assert!(value.is_none() || value.as_deref() == Some("parameter"));
            if binding.write(&mut entity, Some("written".into())).is_ok() {
                assert_eq!(
                    binding.read(&entity, None).unwrap().as_deref(),
                    Some("written")
                );
            }
        }
        Err(_) => {
            binding.invoke_void(&mut entity).unwrap();
            assert_eq!(entity.calls, 1);
        }
    }
}

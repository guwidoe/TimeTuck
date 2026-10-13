// Complete adapted ConfigUtilsTest constructor cases; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/config_instance_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::ConfigInstanceFactory;
fn owner() -> String {
    "ConfigUtilsTest".into()
}
#[test]
fn new_instance_static_inner_class() {
    let value = ConfigInstanceFactory::no_arguments(static_inner)
        .construct(owner, "testProperty")
        .unwrap();
    assert_eq!(
        std::any::type_name_of_val(&value),
        std::any::type_name::<StaticInnerClass>()
    );
}
#[test]
fn new_instance_static_inner_class_with_args_constructor() {
    let error = ConfigInstanceFactory::with_argument(with_args)
        .construct(owner, "testProperty")
        .unwrap_err();
    assert!(error.to_string().contains("no-arg constructor."));
}
#[test]
fn new_instance_non_static_inner_class() {
    let factory = ConfigInstanceFactory::with_context(non_static);
    let error = match factory.construct(owner, "testProperty") {
        Ok(_) => panic!("missing enclosing owner"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("inner class"));
    assert_eq!(
        factory
            .construct_with(
                &ConfigUtilsTest {
                    name: "owner bound"
                },
                owner,
                "testProperty"
            )
            .unwrap()
            .0,
        "owner bound"
    );
}
#[test]
fn new_instance_local_class() {
    #[derive(Debug)]
    struct LocalClass(i32);
    struct LocalContext(i32);
    let factory =
        ConfigInstanceFactory::with_context(|context: &LocalContext| LocalClass(context.0));
    let error = factory.construct(owner, "testProperty").unwrap_err();
    assert!(error.to_string().contains("inner class"));
    assert_eq!(
        factory
            .construct_with(&LocalContext(27), owner, "testProperty")
            .unwrap()
            .0,
        27
    );
}
#[test]
fn new_instance_owner_description() {
    let factory = ConfigInstanceFactory::with_argument(with_args);
    assert!(
        factory
            .construct(owner, "testProperty")
            .unwrap_err()
            .to_string()
            .contains("ConfigUtilsTest")
    );
    let owner = "OWNER";
    assert!(
        factory
            .construct(|| owner.into(), "testProperty")
            .unwrap_err()
            .to_string()
            .contains(owner)
    );
}

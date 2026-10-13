use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::{ConfigInstanceFactory, PlanningType};
static CALLS: AtomicUsize = AtomicUsize::new(0);
struct Arguments(Vec<i32>);
fn construct(value: &Arguments) -> Vec<i32> {
    CALLS.fetch_add(1, Ordering::Relaxed);
    value.0.clone()
}
#[test]
fn real_arguments_validate_before_call_and_each_construct_is_owned() {
    let factory = ConfigInstanceFactory::with_argument(construct);
    let before = CALLS.load(Ordering::Relaxed);
    let error = factory
        .construct_with(&"wrong", || "model".into(), "phase")
        .unwrap_err();
    assert_eq!(error.required_input, Some(PlanningType::of::<Arguments>()));
    assert_eq!(error.actual_input, Some(PlanningType::of::<&str>()));
    assert_eq!(CALLS.load(Ordering::Relaxed), before);
    let mut input = Arguments(vec![2, 4]);
    let first = factory
        .construct_with(&input, || panic!("success must not format owner"), "phase")
        .unwrap();
    input.0.push(7);
    let second = factory
        .construct_with(&input, || panic!("success must not format owner"), "phase")
        .unwrap();
    assert_eq!(first, vec![2, 4]);
    assert_eq!(second, vec![2, 4, 7]);
    assert_eq!(CALLS.load(Ordering::Relaxed), before + 2);
}
#[test]
fn context_uses_current_owner_and_failures_format_owner_once() {
    struct Owner {
        name: String,
    }
    let factory = ConfigInstanceFactory::with_context(|owner: &Owner| owner.name.clone());
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let error = factory
        .construct(
            || {
                observed.fetch_add(1, Ordering::Relaxed);
                "current owner".into()
            },
            "strategy",
        )
        .unwrap_err();
    assert!(error.requires_context);
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert!(error.to_string().contains("strategy"));
    let mut owner = Owner {
        name: "first".into(),
    };
    assert_eq!(
        factory
            .construct_with(&owner, || panic!(), "strategy")
            .unwrap(),
        "first"
    );
    owner.name = "second".into();
    assert_eq!(
        factory
            .construct_with(&owner, || panic!(), "strategy")
            .unwrap(),
        "second"
    );
}
#[test]
fn no_argument_constructor_allocates_each_time_and_rejects_supplied_input() {
    let factory = ConfigInstanceFactory::no_arguments(|| Arc::new(vec![1, 2, 3]));
    let first = factory.construct(|| panic!(), "policy").unwrap();
    let second = factory.construct(|| panic!(), "policy").unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(first, second);
    assert!(
        factory
            .construct_with(&5, || "root".into(), "policy")
            .is_err()
    );
}

// Adapted from pinned Timefold Community lookup tests/fixtures, Apache-2.0.
// Explicit native ID schemas replace annotation discovery; see compat/provenance.json.
use std::sync::Arc;
use timetuck::*;
#[derive(Debug)]
struct IntegerId {
    id: Option<i32>,
}
#[derive(Debug)]
struct PrimitiveId {
    id: i32,
}
#[derive(Debug)]
struct NoId {
    id: i32,
}
#[derive(Debug)]
struct MultipleIds {
    id: i32,
    name: &'static str,
    flag: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Key {
    Int(i32),
    Text(&'static str),
    Bool(bool),
}
#[derive(Debug)]
enum ObjectEnum {
    First,
    Second,
    Third,
}
fn integer(strategy: WorkingLookupStrategy) -> WorkingObjectLookup<IntegerId, i32> {
    WorkingObjectLookup::new(
        strategy,
        WorkingLookupSchema::id("id", |o: &IntegerId| o.id),
    )
}
fn primitive(strategy: WorkingLookupStrategy) -> WorkingObjectLookup<PrimitiveId, i32> {
    WorkingObjectLookup::new(
        strategy,
        WorkingLookupSchema::id("id", |o: &PrimitiveId| Some(o.id)),
    )
}
fn no_id(strategy: WorkingLookupStrategy) -> (WorkingObjectLookup<NoId, i32>, Arc<NoId>) {
    let object = Arc::new(NoId { id: 0 });
    assert_eq!(object.id, 0);
    (
        WorkingObjectLookup::new(strategy, WorkingLookupSchema::without_id()),
        object,
    )
}
fn multiple(
    strategy: WorkingLookupStrategy,
) -> (WorkingObjectLookup<MultipleIds, Key>, Arc<MultipleIds>) {
    (
        WorkingObjectLookup::new(
            strategy,
            WorkingLookupSchema::PlanningIds(vec![
                PlanningIdMember::new("id", |o: &MultipleIds| Some(Key::Int(o.id))),
                PlanningIdMember::new("name", |o: &MultipleIds| Some(Key::Text(o.name))),
                PlanningIdMember::new("bool", |o: &MultipleIds| Some(Key::Bool(o.flag))),
            ]),
        ),
        Arc::new(MultipleIds {
            id: 0,
            name: "",
            flag: false,
        }),
    )
}

#[test]
fn id_or_fail_add_remove_with_integer_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_fail_add_remove_with_primitive_int_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = primitive(S);
    let object = Arc::new(PrimitiveId { id: 0 });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_fail_add_remove_enum() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let object = Arc::new(
        [ObjectEnum::First, ObjectEnum::Second, ObjectEnum::Third]
            .into_iter()
            .next_back()
            .unwrap(),
    );
    let mut l = WorkingObjectLookup::<_, i32>::new(S, WorkingLookupSchema::Immutable);
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
}

#[test]
fn id_or_fail_add_with_null_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: None });
    assert!(matches!(
        l.add(object),
        Err(WorkingLookupError::NullId { member: "id", .. })
    ));
}

#[test]
fn id_or_fail_remove_with_null_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: None });
    assert!(matches!(
        l.remove(&object),
        Err(WorkingLookupError::NullId { member: "id", .. })
    ));
}

#[test]
fn id_or_fail_add_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let (mut l, object) = no_id(S);
    assert!(matches!(
        l.add(object.clone()),
        Err(WorkingLookupError::MissingIdMetadata { .. })
    ));
}

#[test]
fn id_or_fail_remove_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let (mut l, object) = no_id(S);
    assert!(matches!(
        l.remove(&object),
        Err(WorkingLookupError::MissingIdMetadata { .. })
    ));
}

#[test]
fn id_or_fail_add_same_id_twice() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(2) });
    l.add(object.clone()).unwrap();
    let second = Arc::new(IntegerId { id: Some(2) });
    assert_eq!(
        l.add(second).unwrap_err(),
        WorkingLookupError::DuplicateId(2)
    );
    assert!(Arc::ptr_eq(&l.lookup(&object).unwrap(), &object));
}

#[test]
fn id_or_fail_remove_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    assert_eq!(
        l.remove(&object).unwrap_err(),
        WorkingLookupError::DifferentWorkingObject(0)
    );
}

#[test]
fn id_or_fail_look_up_with_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(1) });
    l.add(object.clone()).unwrap();
    let external = Arc::new(IntegerId { id: Some(1) });
    assert!(Arc::ptr_eq(&l.lookup(&external).unwrap(), &object));
    assert!(!Arc::ptr_eq(&external, &object));
}

#[test]
fn id_or_fail_look_up_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let (mut l, object) = no_id(S);
    assert!(l.add(object.clone()).is_err());
    assert!(matches!(
        l.lookup(&object),
        Err(WorkingLookupError::MissingIdMetadata { .. })
    ));
}

#[test]
fn id_or_fail_look_up_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_fail_add_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let (mut l, object) = multiple(S);
    let error = l.add(object.clone()).unwrap_err();
    assert!(
        matches!(&error,WorkingLookupError::MultipleIdMembers {members,..} if members==&vec!["id","name","bool"])
    );
    assert!(error.to_string().contains("3 planning ID members"));
}

#[test]
fn id_or_fail_remove_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrFail;
    let (mut l, object) = multiple(S);
    let error = l.remove(&object).unwrap_err();
    assert!(
        matches!(&error,WorkingLookupError::MultipleIdMembers {members,..} if members==&vec!["id","name","bool"])
    );
    assert!(error.to_string().contains("3 planning ID members"));
}

#[test]
fn id_or_none_add_remove_with_integer_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_none_add_remove_with_primitive_int_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = primitive(S);
    let object = Arc::new(PrimitiveId { id: 0 });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_none_remove_with_null_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: None });
    assert!(matches!(
        l.remove(&object),
        Err(WorkingLookupError::NullId { member: "id", .. })
    ));
}

#[test]
fn id_or_none_add_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let (mut l, object) = no_id(S);
    l.add(object.clone()).unwrap();
}

#[test]
fn id_or_none_remove_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let (mut l, object) = no_id(S);
    l.remove(&object).unwrap();
}

#[test]
fn id_or_none_add_same_id_twice() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(2) });
    l.add(object.clone()).unwrap();
    let second = Arc::new(IntegerId { id: Some(2) });
    assert_eq!(
        l.add(second).unwrap_err(),
        WorkingLookupError::DuplicateId(2)
    );
    assert!(Arc::ptr_eq(&l.lookup(&object).unwrap(), &object));
}

#[test]
fn id_or_none_remove_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    assert_eq!(
        l.remove(&object).unwrap_err(),
        WorkingLookupError::DifferentWorkingObject(0)
    );
}

#[test]
fn id_or_none_look_up_with_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(1) });
    l.add(object.clone()).unwrap();
    let external = Arc::new(IntegerId { id: Some(1) });
    assert!(Arc::ptr_eq(&l.lookup(&external).unwrap(), &object));
    assert!(!Arc::ptr_eq(&external, &object));
}

#[test]
fn id_or_none_look_up_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let (mut l, object) = no_id(S);
    l.add(object.clone()).unwrap();
    assert!(matches!(
        l.lookup(&object),
        Err(WorkingLookupError::Unsupported { .. })
    ));
}

#[test]
fn id_or_none_look_up_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    assert_eq!(
        l.lookup(&object).unwrap_err(),
        WorkingLookupError::UnknownId(0)
    );
}

#[test]
fn id_or_none_add_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let (mut l, object) = multiple(S);
    let error = l.add(object.clone()).unwrap_err();
    assert!(
        matches!(&error,WorkingLookupError::MultipleIdMembers {members,..} if members==&vec!["id","name","bool"])
    );
    assert!(error.to_string().contains("3 planning ID members"));
}

#[test]
fn id_or_none_remove_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::IdOrNone;
    let (mut l, object) = multiple(S);
    let error = l.remove(&object).unwrap_err();
    assert!(
        matches!(&error,WorkingLookupError::MultipleIdMembers {members,..} if members==&vec!["id","name","bool"])
    );
    assert!(error.to_string().contains("3 planning ID members"));
}

#[test]
fn disabled_add_remove_with_integer_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
}

#[test]
fn disabled_add_remove_with_primitive_int_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = primitive(S);
    let object = Arc::new(PrimitiveId { id: 0 });
    l.add(object.clone()).unwrap();
    l.remove(&object).unwrap();
}

#[test]
fn disabled_add_with_null_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: None });
    l.add(object).unwrap();
}

#[test]
fn disabled_remove_with_null_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: None });
    l.remove(&object).unwrap();
}

#[test]
fn disabled_add_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let (mut l, object) = no_id(S);
    l.add(object.clone()).unwrap();
}

#[test]
fn disabled_remove_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let (mut l, object) = no_id(S);
    l.remove(&object).unwrap();
}

#[test]
fn disabled_add_same_id_twice() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(2) });
    l.add(object.clone()).unwrap();
    let second = Arc::new(IntegerId { id: Some(2) });
    l.add(second).unwrap();
}

#[test]
fn disabled_remove_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    l.remove(&object).unwrap();
}

#[test]
fn disabled_look_up_with_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let mut l = integer(S);
    let object = Arc::new(IntegerId { id: Some(1) });
    l.add(object.clone()).unwrap();
    let external = Arc::new(IntegerId { id: Some(1) });
    assert!(matches!(
        l.lookup(&external),
        Err(WorkingLookupError::Unsupported { .. })
    ));
}

#[test]
fn disabled_look_up_without_id() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let (mut l, object) = no_id(S);
    l.add(object.clone()).unwrap();
    assert!(matches!(
        l.lookup(&object),
        Err(WorkingLookupError::Unsupported { .. })
    ));
}

#[test]
fn disabled_look_up_without_adding() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let l = integer(S);
    let object = Arc::new(IntegerId { id: Some(0) });
    assert!(matches!(
        l.lookup(&object),
        Err(WorkingLookupError::Unsupported { .. })
    ));
}

#[test]
fn disabled_add_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let (mut l, object) = multiple(S);
    l.add(object.clone()).unwrap();
}

#[test]
fn disabled_remove_with_two_ids() {
    const S: WorkingLookupStrategy = WorkingLookupStrategy::Disabled;
    let (mut l, object) = multiple(S);
    l.remove(&object).unwrap();
}

#[test]
fn manager_look_up_null() {
    let l = integer(WorkingLookupStrategy::IdOrNone);
    assert!(l.lookup_optional(None).unwrap().is_none());
}
#[test]
fn manager_reset_working_objects() {
    let mut l = integer(WorkingLookupStrategy::IdOrNone);
    let o = Arc::new(IntegerId { id: Some(0) });
    let p = Arc::new(IntegerId { id: Some(1) });
    l.reset();
    l.add(o.clone()).unwrap();
    l.add(p.clone()).unwrap();
    assert!(Arc::ptr_eq(
        &l.lookup(&Arc::new(IntegerId { id: Some(0) })).unwrap(),
        &o
    ));
    assert!(Arc::ptr_eq(
        &l.lookup(&Arc::new(IntegerId { id: Some(1) })).unwrap(),
        &p
    ));
    l.remove(&o).unwrap();
    l.remove(&p).unwrap();
    assert!(l.is_empty());
}

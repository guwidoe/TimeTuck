// Adapted AbstractSolutionClonerTest, Apache-2.0; see provenance/notices.
#[path = "fixtures/basic_clone_plan_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
fn assertions<E: EntityData>(mutate: bool) {
    let original = source::<E>();
    let copied = plan::<E>().copy(&accessors::<E>(false), &original).unwrap();
    let clone = &copied.solution;
    assert!(!std::ptr::eq(&original, clone));
    assert_eq!(clone.code, "solution");
    assert!(Arc::ptr_eq(&original.values, &clone.values));
    assert_eq!(clone.score, original.score);
    assert_eq!(clone.entities.len(), 4);
    assert!(!Arc::ptr_eq(&original.entities, &clone.entities));
    for (i, (code, value)) in [("a", "1"), ("b", "1"), ("c", "3"), ("d", "3")]
        .into_iter()
        .enumerate()
    {
        assert!(!Arc::ptr_eq(&original.entities[i], &clone.entities[i]));
        assert_eq!(original.entities[i].code(), code);
        assert_eq!(clone.entities[i].code(), code);
        assert_eq!(clone.entities[i].value().unwrap().0, value);
    }
    assert!(!Arc::ptr_eq(&original.entities[1], &clone.entities[1]));
    if mutate {
        original.entities[1].set_value(original.values[1].clone());
        assert_eq!(original.entities[1].value().unwrap().0, "2");
        assert_eq!(clone.entities[1].value().unwrap().0, "1");
    }
}
#[test]
fn clone_solution() {
    assertions::<PropertyEntity>(true);
}
#[test]
fn clone_field_annotated_solution() {
    assertions::<FieldEntity>(false);
}

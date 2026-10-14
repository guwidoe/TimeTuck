// Complete native validation adaptations; Apache-2.0, fixture NOTICE.
#[path = "fixtures/named_validation_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn failure(r: &NamedModelFactoryRegistry, count: usize) -> NamedModelFactoryError {
    r.prepare(requests(count, None, None))
        .err()
        .expect("preflight error")
}
fn missing_model(count: usize) {
    let r = NamedModelFactoryRegistry::default();
    let err = failure(&r, count);
    assert_eq!(err, NamedModelFactoryError::MissingModels);
    assert!(err.to_string().contains("no registered planning models"));
}
fn ambiguous_models(count: usize) {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(
        BASIC,
        base::schema(),
        vec![basic_recipe(PROVIDER[0], &calls)],
    )
    .unwrap();
    r.register(
        OTHER,
        other_schema(),
        vec![other_recipe("actual-other", &calls)],
    )
    .unwrap();
    let err = failure(&r, count);
    assert_eq!(
        err,
        NamedModelFactoryError::AmbiguousModels {
            solvers: (1..=count).map(|i| format!("solver{i}")).collect(),
            models: vec![BASIC.into(), OTHER.into()]
        }
    );
    for name in [BASIC, OTHER] {
        assert!(err.to_string().contains(name));
    }
    assert_eq!(calls.get(), 0);
}
fn missing_entities(count: usize) {
    let calls = Rc::new(Cell::new(0));
    let schema = Rc::new(PlanningSchema::new::<base::BasicModel>(vec![]).unwrap());
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, schema, vec![basic_recipe(PROVIDER[0], &calls)])
        .unwrap();
    let err = failure(&r, count);
    assert_eq!(err, NamedModelFactoryError::MissingEntities(BASIC.into()));
    assert!(
        err.to_string()
            .contains("no registered genuine planning entities")
    );
    assert_eq!(calls.get(), 0);
}
fn missing_score(count: usize) {
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![]).unwrap();
    let err = failure(&r, count);
    assert!(
        matches!(err,NamedModelFactoryError::MissingScoring{ref model,scoring:None,..} if model==BASIC)
    );
}
fn ambiguous_score(count: usize, keys: [&str; 2], provider: bool) {
    let calls = Rc::new(Cell::new(0));
    let r = catalog(keys, &calls, provider);
    let err = failure(&r, count);
    let mut expected: Vec<String> = keys.into_iter().map(str::to_owned).collect();
    expected.sort();
    assert_eq!(
        err,
        NamedModelFactoryError::AmbiguousScoring {
            model: BASIC.into(),
            solvers: (1..=count).map(|i| format!("solver{i}")).collect(),
            scorings: expected
        }
    );
    for key in keys {
        assert!(err.to_string().contains(key));
    }
    assert_eq!(calls.get(), 0);
}
fn unused_score(keys: [&str; 2], xml: &str, provider: bool) {
    let calls = Rc::new(Cell::new(0));
    let r = catalog(keys, &calls, provider);
    let err = r.prepare(xml_requests(2, xml)).err().unwrap();
    assert_eq!(
        err,
        NamedModelFactoryError::UnusedScoring {
            model: "global".into(),
            scorings: vec![keys[1].into()]
        }
    );
    assert!(err.to_string().contains(keys[1]));
    assert_eq!(calls.get(), 0);
}
#[test]
fn multiple_unused_solution_class() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(
        BASIC,
        base::schema(),
        vec![basic_recipe(PROVIDER[0], &calls)],
    )
    .unwrap();
    r.register(OTHER, other_schema(), vec![]).unwrap();
    let err = r.prepare(xml_requests(2, NORMAL_XML)).err().unwrap();
    assert_eq!(
        err,
        NamedModelFactoryError::UnusedModels(vec![OTHER.into()])
    );
    assert!(err.to_string().contains(OTHER));
    assert_eq!(calls.get(), 0);
}
#[test]
fn unspecified_no_solution_class() {
    missing_model(1);
}
#[test]
fn unspecified_multiple_solution_classes() {
    ambiguous_models(1);
}
#[test]
fn unspecified_no_entity_class() {
    missing_entities(1);
}
#[test]
fn unspecified_no_constraint_class() {
    missing_score(1);
}
#[test]
fn unspecified_multiple_easy_score_constraints() {
    ambiguous_score(1, EASY, false);
}
#[test]
fn unspecified_multiple_constraint_provider_constraints() {
    ambiguous_score(1, PROVIDER, true);
}
#[test]
fn unspecified_multiple_incremental_score_constraints() {
    ambiguous_score(1, INCREMENTAL, false);
}
#[test]
fn multiple_no_solution_class() {
    missing_model(2);
}
#[test]
fn multiple_multiple_solution_classes() {
    ambiguous_models(2);
}
#[test]
fn multiple_no_entity_class() {
    missing_entities(2);
}
#[test]
fn multiple_no_constraint_class() {
    missing_score(2);
}
#[test]
fn multiple_multiple_easy_score_constraints() {
    ambiguous_score(2, EASY, false);
}
#[test]
fn multiple_multiple_constraint_provider_constraints() {
    ambiguous_score(2, PROVIDER, true);
}
#[test]
fn multiple_multiple_incremental_score_constraints() {
    ambiguous_score(2, INCREMENTAL, false);
}
#[test]
fn multiple_unused_easy_score_constraints() {
    unused_score(EASY, EASY_XML, false);
}
#[test]
fn multiple_unused_constraint_provider_constraints() {
    unused_score(PROVIDER, NORMAL_XML, true);
}
#[test]
fn multiple_unused_incremental_score_constraints() {
    unused_score(INCREMENTAL, INCREMENTAL_XML, false);
}

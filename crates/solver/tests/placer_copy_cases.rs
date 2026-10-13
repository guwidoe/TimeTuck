// Complete pinned Community placer copy TEST adaptations, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/placer_copy_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn exercise(c: Config, expected: Vec<String>) {
    let bare: RebuildablePlacer<Queue, Config, PlacementError> =
        RebuildablePlacer::from_session(queue(&c).unwrap());
    let error = match bare.copy() {
        Err(e) => e,
        Ok(_) => panic!("manual copy accepted"),
    };
    assert_eq!(error.to_string(), "The entity placer cannot be copied.");
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    let mut original = RebuildablePlacer::build(c.clone(), move |c: &Config| {
        seen.set(seen.get() + 1);
        queue(c)
    })
    .unwrap();
    calls.set(0);
    open(original.session_mut());
    assert_eq!(next(original.session_mut()), expected);
    let mut copy = original.copy().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(copy.configuration(), Some(&c));
    assert_eq!(copy.session().state(), SelectionCacheState::Idle);
    assert!(copy.session().key_source().events.is_empty());
    assert!(copy.session_mut().children_mut()[0].opens.is_empty());
    open(copy.session_mut());
    assert_eq!(next(copy.session_mut()), expected);
}
#[test]
fn pooled_entity_copy() {
    exercise(
        Config::Pooled {
            moves: vec!["a1".into(), "a2".into(), "b1".into()],
        },
        vec!["a1".into(), "a2".into(), "b1".into()],
    );
}
#[test]
fn queued_entity_copy() {
    exercise(
        Config::Entity {
            entities: vec!["a".into(), "b".into()],
            primary: vec!["1".into(), "2".into(), "3".into()],
            secondary: vec!["8".into(), "9".into()],
        },
        vec![
            "a->1+a->8",
            "a->1+a->9",
            "a->2+a->8",
            "a->2+a->9",
            "a->3+a->8",
            "a->3+a->9",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    );
}
#[test]
fn queued_value_copy() {
    exercise(
        Config::Value {
            entities: vec!["a".into(), "b".into(), "c".into()],
            values: vec!["1".into(), "2".into()],
        },
        vec!["a->1".into(), "b->1".into(), "c->1".into()],
    );
}

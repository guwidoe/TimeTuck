//! Original registry identity, atomic replacement and solver integration checks.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
struct Fact {
    id: Option<u32>,
    label: &'static str,
}
fn fact(id: u32, label: &'static str) -> Arc<Fact> {
    Arc::new(Fact {
        id: Some(id),
        label,
    })
}
fn registry() -> WorkingObjectLookup<Fact, u32> {
    WorkingObjectLookup::new(
        WorkingLookupStrategy::IdOrFail,
        WorkingLookupSchema::id("id", |f: &Fact| {
            assert_ne!(f.id, Some(99), "ID supplier panic");
            f.id
        }),
    )
}
#[test]
fn identity_checks_and_atomic_replacement_preserve_the_previous_working_set() {
    let mut r = registry();
    let a = fact(1, "a");
    let b = fact(2, "b");
    r.reset_with([a.clone(), b.clone()]).unwrap();
    let external = fact(1, "external");
    assert!(Arc::ptr_eq(&r.lookup(&external).unwrap(), &a));
    assert_eq!(
        r.remove(&external).unwrap_err(),
        WorkingLookupError::DifferentWorkingObject(1)
    );
    assert_eq!(
        r.add(a.clone()).unwrap_err(),
        WorkingLookupError::DuplicateId(1)
    );
    assert!(
        r.reset_with([fact(3, "new"), fact(3, "duplicate")])
            .is_err()
    );
    assert!(
        r.reset_with([
            fact(3, "new"),
            Arc::new(Fact {
                id: None,
                label: "invalid"
            })
        ])
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || r.reset_with([fact(3, "new"), fact(99, "panic")])
        ))
        .is_err()
    );
    assert_eq!(r.len(), 2);
    assert!(Arc::ptr_eq(&r.lookup(&external).unwrap(), &a));
    assert!(Arc::ptr_eq(&r.lookup(&b).unwrap(), &b));
    let next = fact(3, "next");
    r.reset_with([next.clone()]).unwrap();
    assert_eq!(r.lookup(&a).unwrap_err(), WorkingLookupError::UnknownId(1));
    assert!(Arc::ptr_eq(&r.lookup(&fact(3, "external")).unwrap(), &next));
    r.reset();
    assert!(r.is_empty());
    assert_eq!(
        r.lookup(&next).unwrap_err(),
        WorkingLookupError::UnknownId(3)
    );
}
#[test]
fn registry_namespaces_and_immutable_or_disabled_policies_are_explicit() {
    let mut first = registry();
    let mut second = registry();
    let a = fact(1, "first");
    let b = fact(1, "second");
    first.add(a.clone()).unwrap();
    second.add(b.clone()).unwrap();
    assert!(Arc::ptr_eq(&first.lookup(&b).unwrap(), &a));
    assert!(Arc::ptr_eq(&second.lookup(&a).unwrap(), &b));
    let mut immutable = WorkingObjectLookup::<_, u32>::new(
        WorkingLookupStrategy::IdOrNone,
        WorkingLookupSchema::Immutable,
    );
    assert!(Arc::ptr_eq(&immutable.lookup(&a).unwrap(), &a));
    immutable.add(a.clone()).unwrap();
    immutable.remove(&b).unwrap();
    assert!(immutable.is_empty());
    let mut disabled = WorkingObjectLookup::new(
        WorkingLookupStrategy::Disabled,
        WorkingLookupSchema::id("panic", |_: &Fact| -> Option<u32> {
            panic!("disabled must not inspect IDs")
        }),
    );
    disabled.add(a.clone()).unwrap();
    disabled.remove(&a).unwrap();
    disabled.reset_with([b]).unwrap();
    assert!(disabled.lookup_optional(None).unwrap().is_none());
    assert!(matches!(
        disabled.lookup(&a),
        Err(WorkingLookupError::Unsupported { .. })
    ));
}
#[test]
fn resolved_external_values_apply_to_actual_scoped_and_stream_solver_models() {
    let mut lookup = registry();
    let first = fact(10, "working");
    let next = fact(20, "working");
    lookup.reset_with([first.clone(), next.clone()]).unwrap();
    let external = fact(20, "external");
    let p = FiniteProblem::new(vec![
        Variable::new(vec![first.clone(), next.clone()]).with_initial(first.clone()),
    ])
    .unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some(lookup.lookup(&external).unwrap()),
    };
    let invalid = Move::Change {
        variable: 0,
        value: Some(external),
    };
    macro_rules! check {
        ($d:ident) => {{
            assert!($d.apply(&invalid).is_err());
            {
                let trial = $d.apply(&movement).unwrap();
                assert!(Arc::ptr_eq(
                    trial.assignment().values()[0].as_ref().unwrap(),
                    &next
                ));
                assert_eq!(trial.score(), SimpleScore::new(20));
            }
            assert!(Arc::ptr_eq(
                $d.assignment().values()[0].as_ref().unwrap(),
                &first
            ));
            assert_eq!($d.score(), SimpleScore::new(10));
            $d.assert_consistent().unwrap();
        }};
    }
    let mut n = ConstraintNetwork::<Arc<Fact>, SimpleScore>::new(1);
    n.add_constraint("id", vec![0], |values| {
        SimpleScore::new(i64::from(values[0].unwrap().id.unwrap()))
    })
    .unwrap();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    check!(d);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<Arc<Fact>>)>();
    b.score("id", s.port(), |(_, v)| {
        SimpleScore::new(v.as_ref().map_or(0, |v| i64::from(v.id.unwrap())))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    check!(d);
}

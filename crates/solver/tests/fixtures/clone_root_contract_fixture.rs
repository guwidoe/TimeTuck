// Adapted GizmoSolutionClonerTest.cloneExtendedSolution TEST fixture contract.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "extended_clone_fixture.rs"]
pub mod extended;
use extended::*;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};
use timetuck::*;
pub type Base = BaseRoot<AnnotatedBase>;
pub type ChildEntity = Child<AnnotatedBase>;
pub type Extended = ExtendedRoot<AnnotatedBase>;
pub fn builder(calls: &Cell<usize>) -> ObjectClonePlanBuilder<'_> {
    let mut b = ObjectClonePlanBuilder::new();
    b.factory::<ChildEntity>(
        vec![PlanningType::of::<ChildEntity>()],
        move |s| {
            calls.set(calls.get() + 1);
            Ok(Child {
                base: AnnotatedBase::new(s.base.code(), s.base.value()),
                extra: Mutex::new(None),
            })
        },
        |s, c| c.optional_object(&s.extra.lock().unwrap(), ObjectCloneFieldPolicy::Registered),
        |s, t, c| {
            *t.extra.lock().unwrap() =
                c.optional_object(&s.extra.lock().unwrap(), ObjectCloneFieldPolicy::Registered)?;
            Ok(())
        },
    )
    .unwrap();
    b.factory::<Base>(
        vec![PlanningType::of::<ChildEntity>()],
        move |s| {
            calls.set(calls.get() + 1);
            Ok(Base {
                code: s.code,
                values: s.values.clone(),
                entities: Mutex::new(Arc::new(vec![])),
                score: s.score,
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = Arc::new(c.collection(&s.entities.lock().unwrap())?);
            Ok(())
        },
    )
    .unwrap();
    b.factory::<Extended>(
        vec![PlanningType::of::<Base>()],
        move |s| {
            calls.set(calls.get() + 1);
            Ok(Extended {
                base: Base {
                    code: s.base.code,
                    values: s.base.values.clone(),
                    entities: Mutex::new(Arc::new(vec![])),
                    score: s.base.score,
                },
                extra: Mutex::new(s.extra.lock().unwrap().clone()),
            })
        },
        |s, c| {
            for e in s.base.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.base.entities.lock().unwrap() =
                Arc::new(c.collection(&s.base.entities.lock().unwrap())?);
            Ok(())
        },
    )
    .unwrap();
    b
}
pub fn clear_cycle(root: &Extended) {
    let e = root.base.entities.lock().unwrap();
    *e[2].extra.lock().unwrap() = None;
    *e[3].extra.lock().unwrap() = None;
}

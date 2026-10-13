// Adapted TEST-only deep-clone class dependency shapes, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::{Arc, Mutex};
use timetuck::*;
pub struct ExtraDeepClonedObject {
    pub id: String,
}
pub struct TestdataVariousTypes {
    pub extra: Mutex<Option<Arc<ExtraDeepClonedObject>>>,
}
pub struct AnnotatedTestdataVariousTypes {
    pub base: Mutex<Option<Arc<TestdataVariousTypes>>>,
}
pub struct TestdataDeepCloningEntity {
    pub owned: Mutex<Option<Arc<TestdataVariousTypes>>>,
    pub shared: Mutex<Option<Arc<TestdataVariousTypes>>>,
    pub annotated: Mutex<Option<Arc<AnnotatedTestdataVariousTypes>>>,
}
pub struct TestdataFieldAnnotatedDeepCloningEntity {
    pub shadow: Mutex<Vec<String>>,
}
pub struct TestdataFieldAnnotatedDeepCloningSolution {
    pub entities: Mutex<Vec<Arc<TestdataFieldAnnotatedDeepCloningEntity>>>,
}
pub fn builder() -> ObjectClonePlanBuilder<'static> {
    let mut b = ObjectClonePlanBuilder::new();
    b.factory::<ExtraDeepClonedObject>(
        vec![],
        |s| Ok(ExtraDeepClonedObject { id: s.id.clone() }),
        |_, _| Ok(()),
        |_, _, _| Ok(()),
    )
    .unwrap();
    b.factory::<TestdataVariousTypes>(
        vec![PlanningType::of::<ExtraDeepClonedObject>()],
        |_| {
            Ok(TestdataVariousTypes {
                extra: Mutex::new(None),
            })
        },
        |s, c| c.optional_field(&s.extra.lock().unwrap(), ObjectCloneFieldPolicy::Required),
        |s, t, c| {
            *t.extra.lock().unwrap() =
                c.optional_field(&s.extra.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
            Ok(())
        },
    )
    .unwrap();
    b.factory::<AnnotatedTestdataVariousTypes>(
        vec![PlanningType::of::<TestdataVariousTypes>()],
        |_| {
            Ok(AnnotatedTestdataVariousTypes {
                base: Mutex::new(None),
            })
        },
        |s, c| c.optional_field(&s.base.lock().unwrap(), ObjectCloneFieldPolicy::Required),
        |s, t, c| {
            *t.base.lock().unwrap() =
                c.optional_field(&s.base.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
            Ok(())
        },
    )
    .unwrap();
    b.factory::<TestdataDeepCloningEntity>(
        vec![
            PlanningType::of::<TestdataVariousTypes>(),
            PlanningType::of::<AnnotatedTestdataVariousTypes>(),
        ],
        |s| {
            Ok(TestdataDeepCloningEntity {
                owned: Mutex::new(None),
                shared: Mutex::new(s.shared.lock().unwrap().clone()),
                annotated: Mutex::new(None),
            })
        },
        |s, c| {
            c.optional_field(&s.owned.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
            c.optional_field(
                &s.annotated.lock().unwrap(),
                ObjectCloneFieldPolicy::Required,
            )
        },
        |s, t, c| {
            *t.owned.lock().unwrap() =
                c.optional_field(&s.owned.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
            *t.annotated.lock().unwrap() = c.optional_field(
                &s.annotated.lock().unwrap(),
                ObjectCloneFieldPolicy::Required,
            )?;
            Ok(())
        },
    )
    .unwrap();
    b.factory::<TestdataFieldAnnotatedDeepCloningEntity>(
        vec![],
        |s| {
            Ok(TestdataFieldAnnotatedDeepCloningEntity {
                shadow: Mutex::new(s.shadow.lock().unwrap().clone()),
            })
        },
        |_, _| Ok(()),
        |_, _, _| Ok(()),
    )
    .unwrap();
    b.factory::<TestdataFieldAnnotatedDeepCloningSolution>(
        vec![PlanningType::of::<TestdataFieldAnnotatedDeepCloningEntity>()],
        |_| {
            Ok(TestdataFieldAnnotatedDeepCloningSolution {
                entities: Mutex::new(vec![]),
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = c.collection(&s.entities.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    b
}

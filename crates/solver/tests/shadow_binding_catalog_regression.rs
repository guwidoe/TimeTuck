//! Original allocation-scaling and binding-boundary regression checks.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::shadow::<i64>("result"),
    ])])
    .unwrap()
}
struct Key {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Key {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Key {}
#[test]
fn bindings_share_one_catalog_instead_of_cloning_all_keys_per_binding() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("result")
        .unwrap();
    let clones = Rc::new(Cell::new(0));
    let mut b = ShadowSessionBuilder::<Key, ()>::new(&schema);
    let nodes = (0..256)
        .map(|id| {
            b.derive(
                field,
                Key {
                    id,
                    clones: clones.clone(),
                },
                |_, i| {
                    Some(
                        i.for_field::<i64>(field)
                            .unwrap()
                            .first()
                            .copied()
                            .flatten()
                            .copied()
                            .unwrap_or(5),
                    )
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for target in &nodes[1..] {
        let source = nodes[0].erase();
        b.bind_dependencies(target, move |_| Ok(vec![source.clone()]))
            .unwrap();
    }
    assert_eq!(
        clones.get(),
        256,
        "catalog copies keys once for all 255 bindings"
    );
    let mut session = b.build();
    session.update(&()).unwrap();
    for node in nodes {
        assert_eq!(session.read(&node).unwrap().value, Some(&5));
    }
}
#[test]
fn later_registration_gets_new_catalog_without_widening_earlier_binding() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("result")
        .unwrap();
    let selected = RefCell::new(None::<ShadowNodeRef>);
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let source = b.derive(field, 0, |_, _| Some(7i64)).unwrap();
    let old = b
        .derive(field, 1, |_, i| {
            i.for_field::<i64>(field)
                .unwrap()
                .first()
                .copied()
                .flatten()
                .copied()
        })
        .unwrap();
    *selected.borrow_mut() = Some(source.erase());
    b.bind_dependencies(&old, |_| Ok(vec![selected.borrow().clone().unwrap()]))
        .unwrap();
    let later = b.derive(field, 2, |_, _| Some(9i64)).unwrap();
    let new = b
        .derive(field, 3, |_, i| {
            i.for_field::<i64>(field)
                .unwrap()
                .first()
                .copied()
                .flatten()
                .copied()
        })
        .unwrap();
    let later_ref = later.erase();
    b.bind_dependencies(&new, move |_| Ok(vec![later_ref.clone()]))
        .unwrap();
    let mut session = b.build();
    session.update(&()).unwrap();
    assert_eq!(session.read(&old).unwrap().value, Some(&7));
    assert_eq!(session.read(&new).unwrap().value, Some(&9));
    *selected.borrow_mut() = Some(later.erase());
    assert!(matches!(
        update_shadows_with_policy(
            &mut session,
            &(),
            ShadowConsistencyPolicy::AllowInconsistent
        ),
        Err(ConsistentShadowUpdateError::Session(
            ShadowSessionError::UnknownNode
        ))
    ));
    assert_eq!(session.read(&old).unwrap().value, Some(&7));
    assert_eq!(session.read(&new).unwrap().value, Some(&9));
    *selected.borrow_mut() = Some(source.erase());
    session.update(&()).unwrap();
    assert_eq!(session.read(&old).unwrap().value, Some(&7));
}

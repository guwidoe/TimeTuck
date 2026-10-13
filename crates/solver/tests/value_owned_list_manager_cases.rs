// Adapted pinned Community AbstractSolutionManagerTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
#[derive(Debug)]
struct Owner {
    code: &'static str,
}
#[derive(Debug)]
struct Value {
    code: &'static str,
    owner: RefCell<Option<Rc<Owner>>>,
    index: RefCell<Option<usize>>,
}
#[derive(Clone, Debug)]
struct ValueHandle(Rc<Value>);
impl PartialEq for ValueHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ValueHandle {}
#[test]
fn update_assigned_value_with_null_inverse_relation() {
    let entity = Rc::new(Owner { code: "e1" });
    let assigned = ValueHandle(Rc::new(Value {
        code: "assigned",
        owner: RefCell::new(None),
        index: RefCell::new(None),
    }));
    let p = ListProblem::new(
        vec![assigned.clone()],
        vec![ListEntity::new(vec![assigned.clone()])],
    )
    .unwrap()
    .allowing_unassigned();
    let network = ListConstraintNetwork::<ValueHandle, SimpleScore>::new(1, 1);
    let owners = [entity.clone()];
    let mut solution =
        ManagedSolution::new(p.initial_assignment(), vec![ListBuiltinValues::default()]);
    let mut manager =
        ValueOwnedListSolutionManager::new(&p, &network, &owners, |value, fields, owner| {
            *value.0.owner.borrow_mut() = owner.cloned();
            *value.0.index.borrow_mut() = fields.index;
        })
        .unwrap();
    manager
        .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert!(Rc::ptr_eq(
        assigned.0.owner.borrow().as_ref().unwrap(),
        &entity
    ));
    assert_eq!(*assigned.0.index.borrow(), Some(0));
    assert_eq!(entity.code, "e1");
    assert_eq!(assigned.0.code, "assigned");
}

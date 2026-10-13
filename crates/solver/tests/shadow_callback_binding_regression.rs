//! Original native publication, signature rejection and actual cascade execution.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value(Rc<RefCell<OwnedLocation>>);
type OwnedLocation = (Option<Rc<usize>>, Option<usize>);
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Value {}
#[test]
fn value_owned_updates_preserve_score_identity_and_refresh_current_layout() {
    let a = Value(Rc::new(RefCell::new((None, None))));
    let b = Value(Rc::new(RefCell::new((None, None))));
    let p = ListProblem::new(
        vec![a.clone(), b.clone()],
        vec![
            ListEntity::new(vec![a.clone(), b.clone()]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let owners = [Rc::new(0usize), Rc::new(1usize)];
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(2, 2);
    n.add_constraint(
        "must not score shadows only",
        vec![ListScope::Entity(0)],
        |_| {
            calls.set(calls.get() + 1);
            SimpleScore::new(0)
        },
    )
    .unwrap();
    let mut solution = ManagedSolution::new(
        p.initial_assignment(),
        vec![ListBuiltinValues::default(); 2],
    );
    solution.score = Some(SimpleScore::new(17));
    let writes = Cell::new(0);
    let mut manager = ValueOwnedListSolutionManager::new(&p, &n, &owners, |v, fields, owner| {
        writes.set(writes.get() + 1);
        *v.0.borrow_mut() = (owner.cloned(), fields.index);
    })
    .unwrap();
    manager
        .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(solution.score, Some(SimpleScore::new(17)));
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(a.0.borrow().0.as_ref().unwrap(), &owners[0]));
    assert_eq!(b.0.borrow().1, Some(1));
    manager
        .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(writes.get(), 4);
    solution.assignment = ListAssignment::new(vec![vec![b.clone()], vec![a.clone()]]);
    manager
        .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert!(Rc::ptr_eq(a.0.borrow().0.as_ref().unwrap(), &owners[1]));
    assert_eq!(a.0.borrow().1, Some(0));
    assert!(Rc::ptr_eq(b.0.borrow().0.as_ref().unwrap(), &owners[0]));
    assert_eq!(solution.score, Some(SimpleScore::new(17)));
    assert_eq!(calls.get(), 0);
    solution.assignment = ListAssignment::new(vec![vec![b.clone()], vec![]]);
    manager
        .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert!(a.0.borrow().0.is_none());
    assert_eq!(a.0.borrow().1, None);
    let before = writes.get();
    manager
        .update(&mut solution, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(writes.get(), before);
    assert_eq!(calls.get(), 1);
    solution.assignment = ListAssignment::new(vec![vec![b.clone(), b.clone()], vec![]]);
    assert!(
        manager
            .update(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
            .is_err()
    );
    assert_eq!(writes.get(), before);
}
struct World;
#[test]
fn callbacks_bind_actual_rust_signatures_and_execute_existing_cascade() {
    let s = PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<u32>(vec![
        PlanningVariableDefinition::shadow::<i32>("cascade"),
    ])])
    .unwrap();
    let field = s.entity::<u32>().unwrap().variable("cascade").unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut registry = CascadeCallbackRegistry::default();
    let counter = calls.clone();
    registry.register_other(PlanningType::of::<u32>(), "wrong", move || {
        counter.set(counter.get() + 1);
        99i64
    });
    assert!(registry.bind::<u32, i32>(field, "wrong").is_err());
    assert_eq!(calls.get(), 0);
    assert!(registry.bind::<u32, i32>(field, "missing").is_err());
    assert_eq!(calls.get(), 0);
    let counter = calls.clone();
    registry.register(
        PlanningType::of::<u32>(),
        "update",
        ListCascadeCallback::new(move |c: ListCascadeContext<'_, u32, i32>| {
            counter.set(counter.get() + 1);
            c.position
                .map(|_| c.previous_shadow.copied().unwrap_or(1) + 1)
        }),
    );
    let p = ListProblem::new(vec![1u32, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let a = p.initial_assignment();
    let mut bound = registry.bind::<u32, i32>(field, "update").unwrap();
    let mut cascade = bound.initialize(&p, &a).unwrap();
    assert_eq!(cascade.shadows().get(&1), Some(&Some(2)));
    assert_eq!(cascade.shadows().get(&3), Some(&Some(4)));
    assert_eq!(calls.get(), 3);
    let changed = ListAssignment::new(vec![vec![3, 1, 2]]);
    bound.update(&mut cascade, &changed, &[]).unwrap();
    assert_eq!(cascade.shadows().get(&3), Some(&Some(2)));
    assert_eq!(cascade.shadows().get(&2), Some(&Some(4)));
}

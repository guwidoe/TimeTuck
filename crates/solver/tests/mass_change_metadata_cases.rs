// Adapted from pinned Community MassChangeMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md. Native variable slots
// replace entity identities; metadata/reflection and Java describe() are excluded.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;
// Adapted TestdataSolution.generateSolution: cyclic initial values.
fn model(values: usize, entities: usize) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..entities)
            .map(|i| Variable::new((0..values).collect()).with_initial(i % values))
            .collect(),
    )
    .unwrap()
}
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
#[test]
fn execute_sets_every_member_to_destination_value() {
    let p = model(3, 3);
    let mut a = p.initial_assignment();
    let m = mass_change_from_members([0, 1], Some(2));
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(m.into_move()))
        .unwrap();
    assert_eq!(a.values()[0], Some(2));
    assert_eq!(a.values()[1], Some(2));
}
#[test]
fn execute_to_null_unassigns_every_member() {
    let p = model(3, 2);
    let mut a = p.initial_assignment();
    let m = mass_change_from_members([0, 1], None);
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute(|d| d.push(m.into_move()))
        .unwrap();
    assert_eq!(a.values()[0], None);
    assert_eq!(a.values()[1], None);
}
#[test]
fn undo_restores_every_members_original_value() {
    let p = model(3, 3);
    let mut a = p.initial_assignment();
    let original = a.clone();
    let m = mass_change_from_members([0, 1], Some(2));
    MoveTester::new(&p)
        .using(&mut a)
        .unwrap()
        .execute_temporarily(
            |d| d.push(m.into_move()),
            |d| {
                assert_eq!(d.assignment().values()[0], Some(2));
                assert_eq!(d.assignment().values()[1], Some(2));
            },
        )
        .unwrap();
    assert_eq!(a, original);
}
#[test]
fn get_planning_entities_returns_every_member() {
    let m = mass_change_from_members([0, 1], Some("v"));
    assert_eq!(m.members(), [0, 1]);
}
#[test]
fn equals_and_hash_code_ignore_member_draw_order() {
    let a = mass_change_from_members([0, 1], Some("v1"));
    let b = mass_change_from_members([1, 0], Some("v1"));
    let c = mass_change_from_members([0, 1], Some("v2"));
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(a, c);
    // Rust represents null as None and rejects equality with unrelated types.
    assert_ne!(Some(&a), None);
    let unrelated: &dyn std::any::Any = &"not a move";
    assert!(unrelated.downcast_ref::<MassChange<&str>>().is_none());
}
#[test]
fn rebase_returns_move_whose_sample_holds_working_solution_copies() {
    let value = Rc::new("v");
    let m = mass_change_from_members([0, 1], Some(value.clone()));
    let rebased = rebase_mass_change(&m, |i| Ok::<_, ()>(i + 2), |v| Ok(v.clone())).unwrap();
    assert_eq!(rebased.members(), [2, 3]);
    assert!(Rc::ptr_eq(rebased.destination().unwrap(), &value));
    assert_eq!(m.members(), [0, 1]);
}

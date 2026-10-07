// Adapted from pinned Community MassListChangeMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md. Native plans replace Java object moves.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;
fn hash(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
fn not_null_or_unrelated<V: Eq + std::fmt::Debug + 'static>(value: &ListMassChangePlan<V>) {
    assert_ne!(Some(value), None);
    let unrelated: &dyn std::any::Any = &"not a move";
    assert!(unrelated.downcast_ref::<ListMassChangePlan<V>>().is_none());
}
#[test]
fn equals_and_hash_code_ignore_member_draw_order_when_unassigning() {
    let a = ListMassChangePlan::new(["v1", "v2"], None);
    let b = ListMassChangePlan::new(["v2", "v1"], None);
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
    not_null_or_unrelated(&a);
}
#[test]
fn equals_and_hash_code_respect_member_draw_order_when_inserting() {
    let destination = Some(ListPosition {
        entity: 0,
        index: 0,
    });
    let a = ListMassChangePlan::new(["v1", "v2"], destination);
    let same = ListMassChangePlan::new(["v1", "v2"], destination);
    let reversed = ListMassChangePlan::new(["v2", "v1"], destination);
    let shifted = ListMassChangePlan::new(
        ["v1", "v2"],
        Some(ListPosition {
            entity: 0,
            index: 1,
        }),
    );
    let unassign = ListMassChangePlan::new(["v1", "v2"], None);
    assert_eq!(a, same);
    assert_eq!(hash(&a), hash(&same));
    assert_ne!(a, reversed);
    assert_ne!(a, shifted);
    assert_ne!(a, unassign);
    not_null_or_unrelated(&a);
}
#[test]
fn rebase_returns_move_whose_sample_holds_working_solution_copies() {
    let original = [Rc::new("v1"), Rc::new("v2")];
    let working = [Rc::new("v1"), Rc::new("v2")];
    let plan = ListMassChangePlan::new(
        original.clone(),
        Some(ListPosition {
            entity: 0,
            index: 0,
        }),
    );
    let rebased = rebase_list_mass_change_plan(
        &plan,
        |v| Ok::<_, ()>(working[usize::from(Rc::ptr_eq(v, &original[1]))].clone()),
        |e| Ok(e + 1),
    )
    .unwrap();
    assert_eq!(rebased.members().len(), 2);
    for v in &working {
        assert!(rebased.members().iter().any(|got| Rc::ptr_eq(got, v)));
    }
    assert_eq!(
        rebased.destination(),
        Some(ListPosition {
            entity: 1,
            index: 0
        })
    );
}

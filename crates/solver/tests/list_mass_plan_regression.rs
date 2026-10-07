//! Original current-assignment resolution, hash collision and rebasing checks.
use std::{
    cell::Cell,
    collections::HashSet,
    hash::{Hash, Hasher},
};
use timetuck::*;
fn apply(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    plan: &ListMassChangePlan<i32>,
) -> ListAssignment<i32> {
    let mut result = a.clone();
    let m = plan.resolve(p, a).unwrap();
    p.apply(&mut result, &m).unwrap().commit();
    result
}
#[test]
fn recipe_tracks_current_positions_and_unassignment_identity_matches_effects() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let forward = ListMassChangePlan::new([0, 2], None);
    let reversed = ListMassChangePlan::new([2, 0], None);
    assert_eq!(forward, reversed);
    assert_eq!(apply(&p, &a, &forward), apply(&p, &a, &reversed));
    let gather = ListMassChangePlan::new(
        [2, 0],
        Some(ListPosition {
            entity: 1,
            index: 0,
        }),
    );
    assert_eq!(apply(&p, &a, &gather).lists(), &[vec![1], vec![2, 0, 3]]);
    let swap = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    p.apply(&mut a, &swap).unwrap().commit();
    assert_eq!(a.lists(), &[vec![3, 1, 2], vec![0]]);
    assert_eq!(apply(&p, &a, &gather).lists(), &[vec![3, 1], vec![2, 0]]);
    assert_eq!(apply(&p, &a, &forward), apply(&p, &a, &reversed));
}
#[test]
fn unequal_domain_values_with_colliding_hashes_keep_correct_set_identity() {
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct V(u8);
    impl Hash for V {
        fn hash<H: Hasher>(&self, h: &mut H) {
            0u8.hash(h);
        }
    }
    let a = ListMassChangePlan::new([V(1), V(2), V(1)], None);
    let b = ListMassChangePlan::new([V(2), V(1)], None);
    let c = ListMassChangePlan::new([V(1), V(3)], None);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.members().len(), 2);
    assert_eq!(HashSet::from([a, b, c]).len(), 2);
}
#[test]
fn rebase_checks_injective_values_and_preserves_null_without_owner_lookup() {
    let p = ListMassChangePlan::new([0, 1, 0], None);
    let calls = Cell::new(0);
    let r = rebase_list_mass_change_plan(
        &p,
        |v| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(v + 5)
        },
        |_| -> Result<usize, ()> { panic!("null owner") },
    )
    .unwrap();
    assert_eq!(r.members(), [5, 6]);
    assert_eq!(calls.get(), 2);
    assert_eq!(r.destination(), None);
    assert!(matches!(
        rebase_list_mass_change_plan(&p, |_| Ok::<_, ()>(0), Ok),
        Err(MoveRebaseError::Collision(MoveRebaseNamespace::ListValue))
    ));
    assert!(matches!(
        rebase_list_mass_change_plan(&p, |_| Err::<i32, _>("missing"), Ok),
        Err(MoveRebaseError::Lookup("missing"))
    ));
}

//! Original constructor, identity, lookup and model-application regressions.
use std::{cell::Cell, collections::HashSet};
use timetuck::*;
#[test]
fn order_independent_identity_preserves_execution_order_and_deduplicates() {
    let a = mass_change_from_members([3, 1, 3, 2, 1], Some(4));
    let b = mass_change_from_members([2, 3, 1], Some(4));
    assert_eq!(a.members(), [3, 1, 2]);
    assert_eq!(a, b);
    let Move::Composite(steps) = a.movement() else {
        panic!("composite")
    };
    assert_eq!(
        steps,
        &[
            Move::Change {
                variable: 3,
                value: Some(4)
            },
            Move::Change {
                variable: 1,
                value: Some(4)
            },
            Move::Change {
                variable: 2,
                value: Some(4)
            }
        ]
    );
    assert_eq!(HashSet::from([a, b]).len(), 1);
}
#[test]
fn rebase_callbacks_collisions_failures_empty_and_null_metadata() {
    let variables = Cell::new(0);
    let values = Cell::new(0);
    let m = mass_change_from_members([2, 0, 2], Some(5));
    let r = rebase_mass_change(
        &m,
        |i| {
            variables.set(variables.get() + 1);
            Ok::<_, ()>(i + 7)
        },
        |v| {
            values.set(values.get() + 1);
            Ok(v.to_string())
        },
    )
    .unwrap();
    assert_eq!(variables.get(), 2);
    assert_eq!(values.get(), 1);
    assert_eq!(r.members(), [9, 7]);
    assert_eq!(r.destination().map(String::as_str), Some("5"));
    assert!(matches!(
        rebase_mass_change(&m, |_| Ok::<_, ()>(0), |_| Ok(0)),
        Err(MoveRebaseError::Collision(
            MoveRebaseNamespace::BasicVariable
        ))
    ));
    assert!(matches!(
        rebase_mass_change(&m, |_| Err::<usize, _>("lookup"), |_| Ok(0)),
        Err(MoveRebaseError::Lookup("lookup"))
    ));
    let n = mass_change_from_members([0], None::<i32>);
    let r = rebase_mass_change(&n, Ok::<_, ()>, |_| -> Result<i32, ()> {
        panic!("null lookup")
    })
    .unwrap();
    assert_eq!(r.destination(), None);
    let e = mass_change_from_members([], Some(2));
    let r = rebase_mass_change(
        &e,
        |_| -> Result<usize, ()> { panic!("empty member lookup") },
        |v| Ok(v + 1),
    )
    .unwrap();
    assert!(r.members().is_empty());
    assert_eq!(r.destination(), Some(&3));
}

#[test]
fn selected_and_explicit_mass_changes_share_identity() {
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
    ])
    .unwrap();
    let assignment = problem.initial_assignment();
    let selected = mass_change_moves(&problem, &assignment, MassChangeConfig::all())
        .unwrap()
        .find(|m| m.destination() == Some(&2))
        .unwrap();
    let explicit = mass_change_from_members([1, 0], Some(2));
    assert_eq!(selected, explicit);
    assert_ne!(selected.movement(), explicit.movement());
    assert_eq!(HashSet::from([selected, explicit]).len(), 1);
}

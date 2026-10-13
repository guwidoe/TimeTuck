// Original native regressions; no upstream method credit.
use std::borrow::Cow;
use timetuck::*;

fn change(variable: usize, value: i32) -> Move<i32> {
    Move::Change {
        variable,
        value: Some(value),
    }
}

#[test]
fn selector_order_short_circuit_and_errors() {
    let movement = Move::Composite(vec![
        Move::Composite(vec![]),
        change(0, 0),
        Move::Composite(vec![change(1, 1), change(2, 2)]),
        change(3, 3),
    ]);
    let mut seen = vec![];
    assert!(
        movement
            .try_selector_is_doable(|leaf| {
                let Move::Change { variable, .. } = leaf else {
                    panic!("primitive expected")
                };
                seen.push(*variable);
                Ok::<_, ()>(*variable == 1)
            })
            .unwrap()
    );
    assert_eq!(seen, [0, 1]);
    seen.clear();
    assert_eq!(
        movement.try_selector_is_doable(|leaf| {
            let Move::Change { variable, .. } = leaf else {
                panic!("primitive expected")
            };
            seen.push(*variable);
            if *variable == 1 {
                Err("context failed")
            } else {
                Ok(false)
            }
        }),
        Err("context failed")
    );
    assert_eq!(seen, [0, 1]);
}

#[test]
fn selector_eligibility_does_not_replace_final_assignment_validation() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let assignment = problem.initial_assignment();
    let checker = problem.move_checker(&assignment).unwrap();
    let restoring = Move::Composite(vec![change(0, 1), change(0, 0)]);
    assert!(
        restoring
            .try_selector_is_doable(|leaf| checker.is_doable(leaf))
            .unwrap()
    );
    assert!(!checker.is_doable(&restoring).unwrap());
    assert_eq!(assignment.values(), &[Some(0)]);
}

#[test]
fn rebasing_borrows_empty_trees_and_maps_real_leaves() {
    let empty = Move::<i32>::Composite(vec![Move::Composite(vec![])]);
    let borrowed = empty
        .rebase_preserving_no_change(
            |_| -> Result<usize, ()> { panic!("no variables") },
            |_| -> Result<i32, ()> { panic!("no values") },
        )
        .unwrap();
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert!(std::ptr::eq(borrowed.as_ref(), &empty));
    let movement = Move::Composite(vec![change(0, 7), Move::Swap { left: 0, right: 1 }]);
    let rebased = movement
        .rebase_preserving_no_change(|i| Ok::<_, ()>(i + 4), |v| Ok(v + 10))
        .unwrap();
    assert!(matches!(rebased, Cow::Owned(_)));
    assert_eq!(
        rebased.as_ref(),
        &Move::Composite(vec![change(4, 17), Move::Swap { left: 4, right: 5 }])
    );
    assert_eq!(
        movement.rebase_preserving_no_change(|_| Err::<usize, _>("lookup failed"), |v| Ok(*v)),
        Err(MoveRebaseError::Lookup("lookup failed"))
    );
    assert_eq!(
        movement.rebase_preserving_no_change(|_| Ok::<_, ()>(4), |v| Ok(*v)),
        Err(MoveRebaseError::Collision(
            MoveRebaseNamespace::BasicVariable
        ))
    );
}

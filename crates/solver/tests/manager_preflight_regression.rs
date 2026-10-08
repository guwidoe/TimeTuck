//! Original composition checks at the model-validation boundary.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Route;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Route>(vec![
        PlanningVariableDefinition::list::<i32>("visits", false),
    ])])
    .unwrap()
}
#[test]
fn appended_checks_preserve_model_validation_order_and_atomicity_in_all_policies() {
    let s = schema();
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let seen = RefCell::new(Vec::new());
    let failure = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, ListAssignment<i32>>::new(&s);
    let node = b
        .list_input(
            s.entity::<Route>().unwrap().variable("visits").unwrap(),
            0,
            |a| {
                seen.borrow_mut().push("read");
                a.list(0).unwrap().to_vec()
            },
        )
        .unwrap();
    let mut manager = list_declarative_solution_manager(
        &p,
        b.build(),
        |_, _| {
            seen.borrow_mut().push("project");
            Ok::<_, &'static str>(())
        },
        |_, _| {
            seen.borrow_mut().push("validate");
            Ok(())
        },
        |_, _| {
            seen.borrow_mut().push("score");
            Ok(5i64)
        },
    )
    .and_preflight(|_| {
        seen.borrow_mut().push("first");
        if failure.get() == 1 {
            return Err(ModelShadowError::Adapter("first"));
        }
        Ok(())
    })
    .and_preflight(|_| {
        seen.borrow_mut().push("second");
        assert_ne!(failure.get(), 3, "preflight panic");
        if failure.get() == 2 {
            return Err(ModelShadowError::Adapter("second"));
        }
        Ok(())
    });
    let mut solution = ManagedSolution::new(
        ListAssignment::new(vec![vec![1, 1]]),
        ListDerivedShadows {
            builtin: vec![],
            derived: (),
        },
    );
    let policies = [
        SolutionUpdatePolicy::Everything,
        SolutionUpdatePolicy::ShadowsOnly,
        SolutionUpdatePolicy::ScoreOnly,
    ];
    // Factory validation still runs before user checks or readers.
    for policy in policies {
        assert!(matches!(
            manager.update_with_policy(&mut solution, policy),
            Err(ShadowSolutionError::Adapter(ModelShadowError::Model(_)))
        ));
        assert!(seen.borrow().is_empty());
        assert!(matches!(
            manager.session().read(&node),
            Err(ShadowSessionError::NotCurrent)
        ));
    }
    solution.assignment = p.initial_assignment();
    failure.set(1);
    assert!(matches!(
        manager.update(&mut solution),
        Err(ShadowSolutionError::Adapter(ModelShadowError::Adapter(
            "first"
        )))
    ));
    assert_eq!(*seen.borrow(), ["first"]);
    assert!(matches!(
        manager.session().read(&node),
        Err(ShadowSessionError::NotCurrent)
    ));
    assert!(solution.shadows.builtin.is_empty());
    assert_eq!(solution.score, None);
    failure.set(0);
    seen.borrow_mut().clear();
    manager.update(&mut solution).unwrap();
    assert_eq!(
        *seen.borrow(),
        ["first", "second", "read", "project", "validate", "score"]
    );
    let before = solution.clone();
    for policy in policies {
        for stage in 1..=3 {
            failure.set(stage);
            seen.borrow_mut().clear();
            let result = catch_unwind(AssertUnwindSafe(|| {
                manager
                    .update_with_policy(&mut solution, policy)
                    .map(|_| ())
            }));
            if stage == 3 {
                assert!(result.is_err());
            } else {
                assert!(
                    matches!(result.unwrap(),Err(ShadowSolutionError::Adapter(ModelShadowError::Adapter(e))) if e == if stage==1 {"first"} else {"second"})
                );
            }
            assert_eq!(
                seen.borrow().as_slice(),
                if stage == 1 {
                    &["first"][..]
                } else {
                    &["first", "second"][..]
                }
            );
            assert_eq!(solution, before);
            assert_eq!(
                manager.session().read(&node).unwrap().value.unwrap(),
                &[1, 2]
            );
        }
    }
    failure.set(0);
    seen.borrow_mut().clear();
    manager
        .update_with_policy(&mut solution, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(*seen.borrow(), ["first", "second", "validate", "score"]);
}
#[test]
fn explicit_replacement_discards_prior_chain_and_append_works_without_initial_check() {
    let s = schema();
    let order = RefCell::new(Vec::new());
    let mut b = ShadowSessionBuilder::<usize, ()>::new(&s);
    b.list_input(
        s.entity::<Route>().unwrap().variable("visits").unwrap(),
        0,
        |_| vec![1i32],
    )
    .unwrap();
    let mut manager = ShadowSolutionManager::new(
        b.build(),
        |_, _| Ok::<_, &'static str>(()),
        |_, _| Ok(()),
        |_, _| Ok(7),
    )
    .and_preflight(|_| {
        order.borrow_mut().push(1);
        Ok(())
    })
    .and_preflight(|_| {
        order.borrow_mut().push(2);
        Ok(())
    });
    let mut solution = ManagedSolution::new((), ());
    manager.update(&mut solution).unwrap();
    assert_eq!(*order.borrow(), [1, 2]);
    order.borrow_mut().clear();
    manager = manager
        .with_preflight(|_| {
            order.borrow_mut().push(3);
            Ok(())
        })
        .and_preflight(|_| {
            order.borrow_mut().push(4);
            Ok(())
        });
    manager.update(&mut solution).unwrap();
    assert_eq!(*order.borrow(), [3, 4]);
}

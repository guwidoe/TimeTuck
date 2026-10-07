// Original refresh-policy and native-backend integration checks.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;

#[test]
fn policies_order_callbacks_and_support_nonclone_owned_outputs() {
    #[derive(Debug, PartialEq)]
    struct Input(i32);
    #[derive(Debug, PartialEq)]
    struct Shadow(i32);
    #[derive(Debug, PartialEq)]
    struct ScoreValue(i32);
    let calls = RefCell::new(vec![]);
    let mut manager = SolutionManager::new(
        |a: &Input| {
            calls.borrow_mut().push("shadow");
            Ok::<_, ()>(Shadow(a.0 * 3))
        },
        |_: &Input, h: &Shadow| {
            calls.borrow_mut().push("validate");
            assert!(h.0 >= 0);
            Ok(())
        },
        |a: &Input, h: &Shadow| {
            calls.borrow_mut().push("score");
            Ok(ScoreValue(a.0 + h.0))
        },
    );
    let mut s = ManagedSolution::new(Input(4), Shadow(0));
    assert_eq!(
        manager
            .update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
            .unwrap(),
        None
    );
    assert_eq!(s.shadows, Shadow(12));
    assert_eq!(*calls.borrow(), ["shadow", "validate"]);
    calls.borrow_mut().clear();
    assert_eq!(
        manager
            .update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
            .unwrap(),
        Some(&ScoreValue(16))
    );
    assert_eq!(*calls.borrow(), ["validate", "score"]);
    calls.borrow_mut().clear();
    s.assignment = Input(5);
    assert_eq!(
        manager
            .update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
            .unwrap(),
        Some(&ScoreValue(16))
    );
    assert_eq!(s.shadows, Shadow(15));
    calls.borrow_mut().clear();
    assert_eq!(manager.update(&mut s).unwrap(), Some(&ScoreValue(20)));
    assert_eq!(*calls.borrow(), ["shadow", "validate", "score"]);
}

#[test]
fn each_callback_error_or_panic_leaves_all_published_data_unchanged() {
    let failure = Rc::new(Cell::new(0u8));
    let check = |stage| {
        let f = failure.clone();
        move || {
            let code = f.get();
            assert_ne!(code, stage + 3, "injected panic");
            if code == stage { Err(stage) } else { Ok(()) }
        }
    };
    let refresh = check(1);
    let validate = check(2);
    let score = check(3);
    let mut m = SolutionManager::new(
        move |a: &i32| {
            refresh()?;
            Ok(a * 3)
        },
        move |_, _| validate(),
        move |a: &i32, h: &i32| {
            score()?;
            Ok(a + h)
        },
    );
    let original = ManagedSolution {
        assignment: 5,
        shadows: 99,
        score: Some(-1),
    };
    for code in 1..=6 {
        let mut state = original.clone();
        failure.set(code);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            m.update(&mut state).map(|s| s.copied())
        }));
        if code <= 3 {
            assert_eq!(result.unwrap(), Err(code));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(state, original);
    }
    failure.set(0);
    let mut state = original;
    m.update(&mut state).unwrap();
    assert_eq!(state.shadows, 15);
    assert_eq!(state.score, Some(20));
}

#[test]
fn basic_factory_recalculates_real_constraints_and_initialization() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![2, 7]).with_initial(2),
        Variable::new(vec![2, 7]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<i32, SimpleScore>::new(2);
    n.add_constraint("sum", vec![0, 1], |facts| {
        SimpleScore::new(facts.iter().filter_map(|v| *v).map(|v| i64::from(*v)).sum())
    })
    .unwrap();
    let mut m = basic_solution_manager(&p, &n);
    let mut s = ManagedSolution::new(p.initial_assignment(), ());
    m.update(&mut s).unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(2).with_structural(-1)));
    p.apply(
        &mut s.assignment,
        &Move::Change {
            variable: 1,
            value: Some(7),
        },
    )
    .unwrap()
    .commit();
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(2).with_structural(-1)));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(9)));
    s.assignment = Assignment::new(vec![Some(99), Some(7)]);
    let before = s.clone();
    assert!(m.update(&mut s).is_err());
    assert_eq!(s, before);
}

#[test]
fn list_factory_updates_all_structural_shadows_and_rejects_stale_score_only_inputs() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new(vec![]); 3]).unwrap();
    let mut n = ListConstraintNetwork::<i32, SimpleScore>::new(3, 6);
    n.add_constraint(
        "sum",
        vec![
            ListScope::Entity(0),
            ListScope::Entity(1),
            ListScope::Entity(2),
        ],
        |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(row) = f else {
                            unreachable!()
                        };
                        row.iter().map(|v| i64::from(*v)).sum::<i64>()
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    let mut m = list_solution_manager(&p, &n);
    for offset in 0..6 {
        let values: Vec<_> = (0..4).map(|i| (i + offset) % 6).collect();
        let a = ListAssignment::new(vec![values[..2].to_vec(), values[2..].to_vec(), vec![]]);
        let mut s = ManagedSolution::new(a, vec![]);
        assert!(matches!(
            m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly),
            Err(SolutionRefreshError::ShadowCount {
                expected: 6,
                actual: 0
            })
        ));
        m.update(&mut s).unwrap();
        assert_eq!(
            s.score,
            Some(SimpleScore::new(values.iter().map(|v| i64::from(*v)).sum()).with_structural(-2))
        );
        for v in 0..6 {
            let mut expected = ListBuiltinValues::default();
            for (owner, row) in s.assignment.lists().iter().enumerate() {
                for (index, &current) in row.iter().enumerate() {
                    if current == v {
                        expected = ListBuiltinValues {
                            owner: Some(owner),
                            index: Some(index),
                            previous: row[..index].last().copied(),
                            next: row.get(index + 1).copied(),
                        };
                    }
                }
            }
            assert_eq!(s.shadows[v as usize], expected);
        }
        let repaired = s.clone();
        for field in 0..4 {
            match field {
                0 => s.shadows[0].owner = Some(99),
                1 => s.shadows[0].index = Some(99),
                2 => s.shadows[0].previous = Some(99),
                _ => s.shadows[0].next = Some(99),
            }
            let invalid = s.clone();
            assert_eq!(
                m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly),
                Err(SolutionRefreshError::ShadowMismatch { value: 0 })
            );
            assert_eq!(s, invalid);
            m.update(&mut s).unwrap();
            assert_eq!(s, repaired);
        }
        s.assignment = ListAssignment::new(vec![vec![0, 0], vec![], vec![]]);
        let invalid = s.clone();
        assert!(matches!(
            m.update(&mut s),
            Err(SolutionRefreshError::Backend(_))
        ));
        assert_eq!(s, invalid);
    }
}

#[test]
fn mixed_factory_combines_models_and_preserves_score_on_shadow_only_update() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![3, 8]).with_initial(3),
            Variable::new(vec![3, 8]),
        ])
        .unwrap(),
        ListProblem::new(vec![4, 5, 6], vec![ListEntity::new(vec![4, 6])]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(2, 1, 3);
    n.add_constraint(
        "mixed",
        vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
        |facts| {
            let [MixedFact::Basic(Some(value)), MixedFact::ListEntity(row)] = facts else {
                unreachable!()
            };
            SimpleScore::new(i64::from(**value) + row.iter().map(|v| i64::from(*v)).sum::<i64>())
        },
    )
    .unwrap();
    let mut m = mixed_solution_manager(&p, &n);
    let mut s = ManagedSolution::new(p.initial_assignment(), vec![]);
    m.update(&mut s).unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(13).with_structural(-2)));
    let movement = MixedMove {
        basic: Move::Change {
            variable: 1,
            value: Some(8),
        },
        lists: ListMove::Assign {
            value: 5,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
    };
    p.apply(&mut s.assignment, &movement).unwrap().commit();
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(
        s.shadows[1],
        ListBuiltinValues {
            owner: Some(0),
            index: Some(1),
            previous: Some(4),
            next: Some(6)
        }
    );
    assert_eq!(s.score, Some(SimpleScore::new(13).with_structural(-2)));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(18)));
}

#[test]
fn typed_shadow_session_can_supply_custom_refresh_without_running_search() {
    struct World;
    struct Entity;
    let schema = PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<i64>("doubled"),
    ])])
    .unwrap();
    let input = schema
        .entity::<Entity>()
        .unwrap()
        .variable("input")
        .unwrap();
    let derived = schema
        .entity::<Entity>()
        .unwrap()
        .variable("doubled")
        .unwrap();
    let refresh = |world: &i64| -> Result<i64, ShadowSessionError> {
        let mut b = ShadowSessionBuilder::new(&schema);
        let i = b.input(input, 0, |w: &i64| Some(*w))?;
        let read = i.clone();
        let d = b.derive(derived, 0, move |_, inputs| {
            inputs.get(&read).unwrap().map(|v| v * 2)
        })?;
        b.set_dependencies(&d, &[i.erase()])?;
        let mut session = b.try_build()?;
        session.update(world)?;
        Ok(*session.read(&d)?.value.unwrap())
    };
    let mut manager = SolutionManager::new(
        refresh,
        |_: &i64, _: &i64| Ok(()),
        |a: &i64, h: &i64| Ok(SimpleScore::new(a + h)),
    );
    let mut s = ManagedSolution::new(9i64, 0i64);
    manager.update(&mut s).unwrap();
    assert_eq!(s.shadows, 18);
    assert_eq!(s.score, Some(SimpleScore::new(27)));
    s.assignment = 10;
    manager
        .update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(s.shadows, 18);
    assert_eq!(s.score, Some(SimpleScore::new(28)));
    manager.update(&mut s).unwrap();
    assert_eq!(s.shadows, 20);
    assert_eq!(s.score, Some(SimpleScore::new(30)));
}

#[test]
fn native_score_failure_does_not_publish_prepared_list_shadows() {
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![2, 1])]).unwrap();
    let wrong_shape = ListConstraintNetwork::<i32, SimpleScore>::new(2, 2);
    let mut m = list_solution_manager(&p, &wrong_shape);
    let mut s = ManagedSolution {
        assignment: p.initial_assignment(),
        shadows: vec![ListBuiltinValues::default(); 2],
        score: Some(SimpleScore::new(99)),
    };
    let before = s.clone();
    assert!(matches!(
        m.update(&mut s),
        Err(SolutionRefreshError::Backend(
            ListScoringError::ModelShape { .. }
        ))
    ));
    assert_eq!(s, before);
    // The shadow-only path does not consult the calculator's incompatible shape.
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(s.shadows[0].index, Some(1));
    assert_eq!(s.shadows[1].index, Some(0));
    assert_eq!(s.score, Some(SimpleScore::new(99)));
}

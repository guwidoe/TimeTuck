//! Original integration checks for atomic built-in and calculated list shadows.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Route;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Route>(vec![
        PlanningVariableDefinition::list::<i32>("visits", false),
        PlanningVariableDefinition::basic::<i32>("factor", false),
        PlanningVariableDefinition::shadow::<i64>("cost"),
    ])])
    .unwrap()
}
fn row_cost(row: &[i32]) -> i64 {
    row.iter()
        .enumerate()
        .map(|(i, &v)| (i as i64 + 1) * i64::from(v))
        .sum()
}
#[derive(Debug, PartialEq)]
struct Costs(Vec<i64>); // Projection need not be Clone.
#[test]
fn list_refreshes_structural_and_derived_state_and_rolls_back_score_failures() {
    let schema = schema();
    let route = schema.entity::<Route>().unwrap();
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    let calls = Cell::new(0);
    let failure = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, ListAssignment<i32>>::new(&schema);
    let mut outputs = vec![];
    for entity in 0..2 {
        let input = b
            .list_input(route.variable("visits").unwrap(), entity, move |a| {
                a.list(entity).unwrap().to_vec()
            })
            .unwrap();
        let source = input.clone();
        let calls = &calls;
        let out = b
            .derive(route.variable("cost").unwrap(), entity, move |_, i| {
                calls.set(calls.get() + 1);
                Some(row_cost(i.get(&source).unwrap().unwrap()))
            })
            .unwrap();
        b.set_dependencies(&out, &[input.erase()]).unwrap();
        outputs.push(out);
    }
    let observed = outputs.clone();
    let mut manager = list_declarative_solution_manager(
        &p,
        b.build(),
        move |_, graph| {
            Ok(Costs(
                outputs
                    .iter()
                    .map(|node| *graph.read(node).unwrap().value.unwrap())
                    .collect(),
            ))
        },
        |a, h| {
            assert_eq!(
                h.derived.0,
                a.lists()
                    .iter()
                    .map(|row| row_cost(row))
                    .collect::<Vec<_>>()
            );
            if failure.get() == 3 {
                return Err("validation");
            }
            Ok(())
        },
        |_, h| {
            if failure.get() == 1 {
                return Err("scoring");
            }
            assert_ne!(failure.get(), 2, "scoring panic");
            Ok(SimpleScore::new(-h.derived.0.iter().sum::<i64>()))
        },
    );
    let mut solution = ManagedSolution::new(
        p.initial_assignment(),
        ListDerivedShadows {
            builtin: vec![],
            derived: Costs(vec![]),
        },
    );
    manager.update(&mut solution).unwrap();
    assert_eq!(solution.shadows.derived, Costs(vec![5, 3]));
    assert_eq!(solution.score, Some(SimpleScore::new(-8)));
    assert_eq!(solution.shadows.builtin[3], ListBuiltinValues::default());
    assert_eq!(calls.get(), 2);
    manager.update(&mut solution).unwrap();
    assert_eq!(calls.get(), 2);
    solution.assignment = ListAssignment::new(vec![vec![4, 1], vec![2, 3]]);
    for mode in [1, 2, 3] {
        failure.set(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            manager.update(&mut solution).map(|_| ())
        }));
        if mode == 2 {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ShadowSolutionError::Adapter(ModelShadowError::Adapter(_)))
            ));
        }
        assert_eq!(solution.shadows.derived, Costs(vec![5, 3]));
        assert_eq!(solution.score, Some(SimpleScore::new(-8)));
        assert_eq!(solution.shadows.builtin[3], ListBuiltinValues::default());
        assert_eq!(
            manager.session().read(&observed[0]).unwrap().value,
            Some(&5)
        );
        assert_eq!(
            manager.session().read(&observed[1]).unwrap().value,
            Some(&3)
        );
    }
    failure.set(0);
    manager.update(&mut solution).unwrap();
    assert_eq!(solution.shadows.derived, Costs(vec![6, 8]));
    assert_eq!(solution.score, Some(SimpleScore::new(-14)));
    assert_eq!(solution.shadows.builtin[3].owner, Some(0));
    assert_eq!(solution.shadows.builtin[3].index, Some(0));
    assert_eq!(solution.shadows.builtin[3].next, Some(1));
    assert_eq!(solution.shadows.builtin[0].previous, Some(4));
    assert_eq!(solution.shadows.builtin[1].owner, Some(1));
    assert_eq!(solution.shadows.builtin[2].previous, Some(2));
}
#[test]
fn list_preflight_and_score_only_checks_run_before_domain_callbacks() {
    let schema = schema();
    let route = schema.entity::<Route>().unwrap();
    let problem = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let readers = Cell::new(0);
    let adapters = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, ListAssignment<i32>>::new(&schema);
    let node = builder
        .list_input(route.variable("visits").unwrap(), 0, |a| {
            readers.set(readers.get() + 1);
            a.list(0).unwrap().to_vec()
        })
        .unwrap();
    let mut manager = list_declarative_solution_manager(
        &problem,
        builder.build(),
        |_, _| {
            adapters.set(adapters.get() + 1);
            Ok::<_, &'static str>(())
        },
        |_, _| {
            adapters.set(adapters.get() + 1);
            Ok(())
        },
        |_, _| {
            adapters.set(adapters.get() + 1);
            Ok(7i64)
        },
    );
    let mut solution = ManagedSolution::new(
        ListAssignment::new(vec![vec![1, 1]]),
        ListDerivedShadows {
            builtin: vec![],
            derived: (),
        },
    );
    for policy in [
        SolutionUpdatePolicy::Everything,
        SolutionUpdatePolicy::ShadowsOnly,
        SolutionUpdatePolicy::ScoreOnly,
    ] {
        assert!(matches!(
            manager.update_with_policy(&mut solution, policy),
            Err(ShadowSolutionError::Adapter(ModelShadowError::Model(_)))
        ));
        assert_eq!((readers.get(), adapters.get()), (0, 0));
        assert!(manager.session().read(&node).is_err());
    }
    solution.assignment = problem.initial_assignment();
    manager
        .update_with_policy(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(solution.score, None);
    let before = (readers.get(), adapters.get());
    solution.shadows.builtin[1].previous = Some(999);
    assert!(matches!(
        manager.update_with_policy(&mut solution, SolutionUpdatePolicy::ScoreOnly),
        Err(ShadowSolutionError::Adapter(ModelShadowError::Builtin(
            SolutionRefreshError::ShadowMismatch {
                value: 1,
                mismatch: ListShadowMismatch::Previous {
                    expected: Some(1),
                    actual: Some(999)
                }
            }
        )))
    ));
    assert_eq!((readers.get(), adapters.get()), before);
    solution.shadows.builtin[1].previous = Some(1);
    manager
        .update_with_policy(&mut solution, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(solution.score, Some(7));
    assert_eq!(readers.get(), before.0);
    assert_eq!(adapters.get(), before.1 + 2);
}
#[test]
fn mixed_graph_uses_both_components_and_domain_validator_rejects_stale_custom_outputs() {
    let schema = schema();
    let route = schema.entity::<Route>().unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2, 3]).with_initial(2)]).unwrap(),
        ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap(),
    );
    let readers = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, MixedAssignment<i32, i32>>::new(&schema);
    let input = b
        .list_input(route.variable("visits").unwrap(), 0, |a| {
            a.lists().list(0).unwrap().to_vec()
        })
        .unwrap();
    let factor = b
        .input(route.variable("factor").unwrap(), 0, |a| {
            readers.set(readers.get() + 1);
            a.basic().values()[0]
        })
        .unwrap();
    let left = input.clone();
    let right = factor.clone();
    let cost = b
        .derive(route.variable("cost").unwrap(), 0, move |_, i| {
            Some(
                row_cost(i.get(&left).unwrap().unwrap())
                    * i64::from(*i.get(&right).unwrap().unwrap_or(&0)),
            )
        })
        .unwrap();
    b.set_dependencies(&cost, &[input.erase(), factor.erase()])
        .unwrap();
    let out = cost.clone();
    let mut m = mixed_declarative_solution_manager(
        &p,
        b.build(),
        move |_, graph| Ok(*graph.read(&out).unwrap().value.unwrap()),
        |a, h| {
            let expected = row_cost(a.lists().list(0).unwrap())
                * i64::from(a.basic().values()[0].unwrap_or(0));
            if h.derived != expected {
                Err("stale custom cost")
            } else {
                Ok(())
            }
        },
        |_, h| Ok(SimpleScore::new(-h.derived)),
    );
    let mut s = ManagedSolution::new(
        p.initial_assignment(),
        ListDerivedShadows {
            builtin: vec![],
            derived: 0,
        },
    );
    m.update(&mut s).unwrap();
    assert_eq!(
        (s.shadows.derived, s.score),
        (10, Some(SimpleScore::new(-10)))
    );
    s.assignment =
        MixedAssignment::new(Assignment::new(vec![Some(3)]), s.assignment.lists().clone());
    let before = readers.get();
    assert!(matches!(
        m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly),
        Err(ShadowSolutionError::Adapter(ModelShadowError::Adapter(
            "stale custom cost"
        )))
    ));
    assert_eq!(readers.get(), before);
    assert_eq!(s.score, Some(SimpleScore::new(-10)));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(s.shadows.derived, 15);
    assert_eq!(s.score, Some(SimpleScore::new(-10)));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(s.score, Some(SimpleScore::new(-15)));
    s.assignment = MixedAssignment::new(
        Assignment::new(vec![Some(99)]),
        ListAssignment::new(vec![vec![2, 1]]),
    );
    let before = readers.get();
    assert!(matches!(
        m.update(&mut s),
        Err(ShadowSolutionError::Adapter(ModelShadowError::Model(
            MixedError::Basic(_)
        )))
    ));
    assert_eq!(readers.get(), before);
    assert_eq!(m.session().read(&cost).unwrap().value, Some(&15));
    s.assignment = MixedAssignment::new(
        Assignment::new(vec![Some(1)]),
        ListAssignment::new(vec![vec![2, 1]]),
    );
    m.update(&mut s).unwrap();
    assert_eq!(s.shadows.derived, 4);
    assert_eq!(s.score, Some(SimpleScore::new(-4)));
    assert_eq!(s.shadows.builtin[0].previous, Some(2));
}

#[test]
fn graph_rejection_precedes_projection_and_can_recover_after_dependency_repair() {
    let schema = schema();
    let route = schema.entity::<Route>().unwrap();
    let problem = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let calls = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, ListAssignment<i32>>::new(&schema);
    let cost = b
        .derive(route.variable("cost").unwrap(), 0, |_, _| Some(1i64))
        .unwrap();
    b.set_dependencies(&cost, &[cost.erase()]).unwrap();
    let out = cost.clone();
    let mut m = list_declarative_solution_manager(
        &problem,
        b.build(),
        move |_, graph| Ok::<_, &'static str>(*graph.read(&out).unwrap().value.unwrap()),
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        |_, h| Ok(h.derived),
    );
    let mut s = ManagedSolution::new(
        problem.initial_assignment(),
        ListDerivedShadows {
            builtin: vec![],
            derived: 99,
        },
    );
    for policy in [
        SolutionUpdatePolicy::Everything,
        SolutionUpdatePolicy::ShadowsOnly,
    ] {
        match m.update_with_policy(&mut s, policy).unwrap_err() {
            ShadowSolutionError::Shadows(ConsistentShadowUpdateError::Inconsistent(report)) => {
                assert_eq!(report.loops.len(), 1);
                assert_eq!(report.loops[0][0].variable, "cost");
            }
            error => panic!("{error:?}"),
        }
        assert_eq!(calls.get(), 0);
        assert!(s.shadows.builtin.is_empty());
        assert_eq!(s.shadows.derived, 99);
        assert_eq!(s.score, None);
        assert!(matches!(
            m.session().read(&cost),
            Err(ShadowSessionError::NotCurrent)
        ));
    }
    m.session_mut().set_dependencies(&cost, &[]).unwrap();
    m.update(&mut s).unwrap();
    assert_eq!(s.score, Some(1));
    assert_eq!(s.shadows.derived, 1);
    assert_eq!(s.shadows.builtin[0].owner, Some(0));
    assert_eq!(calls.get(), 1);
}

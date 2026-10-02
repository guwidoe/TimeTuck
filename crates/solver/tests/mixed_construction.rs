//! Original mixed construction checks; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    )
}
fn settings(order: MixedConstructionOrder, budget: u64) -> MixedConstructionConfig {
    MixedConstructionConfig {
        construction: ConstructionConfig {
            max_evaluations: budget,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
        },
        order,
    }
}
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];
fn parts(facts: &[MixedFact<'_, i32, i32>]) -> (Option<i32>, Option<usize>) {
    let MixedFact::Basic(b) = facts[0] else {
        panic!()
    };
    let MixedFact::ListValue { ref element, .. } = facts[1] else {
        panic!()
    };
    (b.copied(), element.as_ref().map(|v| v.position.entity))
}
#[test]
fn shared_budget_counts_both_phases_and_preserves_first_ties_without_rescoring() {
    let p = problem();
    let calls = Cell::new(0);
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "count",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        },
    )
    .unwrap();
    for order in ORDERS {
        for budget in 0..=6 {
            calls.set(0);
            let r = greedy_mixed_construction(&p, &n, settings(order, budget)).unwrap();
            assert_eq!(r.evaluations, budget.min(5));
            assert_eq!(calls.get() as u64, r.evaluations);
            assert_eq!(r.constraint_evaluations, r.evaluations as u128);
            assert_eq!(
                r.termination,
                if budget >= 5 {
                    ConstructionTermination::Completed
                } else {
                    ConstructionTermination::EvaluationLimit
                }
            );
            assert_eq!(r.complete, budget >= 4);
            assert_eq!(r.solution().is_some(), budget >= 4);
            assert_eq!(r.score.is_some(), budget > 0);
            let assigned = usize::from(budget >= 2) + usize::from(budget >= 4);
            assert_eq!(
                r.assigned_basic_variables + r.assigned_list_values,
                assigned
            );
            if budget > 0 {
                assert_eq!(r.score.unwrap().structural(), assigned as i64 - 2);
            }
            if r.assigned_basic_variables > 0 {
                assert_eq!(r.assignment.basic().values(), &[Some(0)]);
            }
            if r.assigned_list_values > 0 {
                assert_eq!(r.assignment.lists().location(&7).unwrap().entity, 0);
            }
            if budget == 2 || budget == 3 {
                match order {
                    MixedConstructionOrder::BasicThenLists => {
                        assert_eq!(r.assigned_basic_variables, 1)
                    }
                    MixedConstructionOrder::ListsThenBasic => assert_eq!(r.assigned_list_values, 1),
                }
            }
        }
    }
}
#[test]
fn construction_order_changes_coupled_quality_and_local_search_can_repair_it() {
    let p = problem();
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    let score = |b, o| match (b, o) {
        (Some(1), Some(1)) => 10,
        (Some(0), _) => 5,
        (None, Some(1)) => 4,
        _ => 0,
    };
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        move |f| {
            let (b, o) = parts(f);
            SimpleScore::new(score(b, o))
        },
    )
    .unwrap();
    for order in ORDERS {
        let mut c = settings(order, 100);
        c.construction.assert_scores = true;
        let r = greedy_mixed_construction(&p, &n, c).unwrap();
        assert!(r.complete);
        assert_eq!(r.evaluations, 5);
        let expected = if order == MixedConstructionOrder::BasicThenLists {
            5
        } else {
            10
        };
        assert_eq!(r.score, Some(SimpleScore::new(expected)));
        assert_eq!(r.score, Some(n.calculate(&p, &r.assignment).unwrap()));
        let local = mixed_local_search_from(
            &p,
            &n,
            r.solution().unwrap().assignment,
            MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 100,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: true,
            },
        )
        .unwrap();
        let exact = exhaustive_mixed_search(
            &p,
            |a| {
                SimpleScore::new(score(
                    a.basic().values()[0],
                    a.lists().location(&7).map(|p| p.entity),
                ))
            },
            u64::MAX,
        );
        assert_eq!(local.best.unwrap().score, exact.best.unwrap().score);
        assert_eq!(p.initial_assignment().basic().values(), &[None]);
    }
}
#[test]
fn initial_values_pins_ranges_and_optional_unassignment_survive_both_orders() {
    for optional in [false, true] {
        for order in ORDERS {
            let basic = FiniteProblem::new(vec![
                Variable::new(vec![0, 1]).with_initial(1),
                Variable::new(vec![0, 1]),
                Variable::new(vec![]).allowing_unassigned(),
                Variable::new(vec![9]).with_initial(9).with_pinned(true),
                Variable::new(vec![])
                    .allowing_unassigned()
                    .with_pinned(true),
            ])
            .unwrap();
            let mut lists = ListProblem::new(
                vec![11, 23, 47, 89],
                vec![
                    ListEntity::new(vec![11, 47])
                        .with_pinned_prefix(1)
                        .with_value_range(vec![11, 23, 47]),
                    ListEntity::new(vec![89]).with_pinned(true),
                ],
            )
            .unwrap();
            if optional {
                lists = lists.allowing_unassigned();
            }
            let p = MixedProblem::new(basic, lists);
            let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(5, 2, 4);
            let original = p.initial_assignment();
            let r = greedy_mixed_construction(&p, &n, settings(order, 100)).unwrap();
            assert!(p.is_complete(&r.assignment));
            assert_eq!(
                r.assignment.basic().values(),
                &[Some(1), Some(0), None, Some(9), None]
            );
            assert_eq!(r.assigned_basic_variables, 1);
            assert_eq!(r.assigned_list_values, usize::from(!optional));
            assert_eq!(
                r.assignment.lists().list(0),
                Some(
                    if optional {
                        vec![11, 47]
                    } else {
                        vec![11, 23, 47]
                    }
                    .as_slice()
                )
            );
            assert_eq!(r.assignment.lists().list(1), Some([89].as_slice()));
            assert_eq!(r.score, Some(SimpleScore::ZERO));
            assert_eq!(p.initial_assignment(), original);
        }
    }
}
#[test]
fn all_early_pick_policies_ignore_partial_structure_and_share_one_budget() {
    let p = problem();
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "hard boundary",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| match parts(f).0 {
            None => HardSoftScore::new(-10, 0),
            Some(0) => HardSoftScore::new(-10, -5),
            _ => HardSoftScore::new(-9, 0),
        },
    )
    .unwrap();
    for order in ORDERS {
        for (pick, eval, basic) in [
            (ConstructionPickEarly::Never, 5, 1),
            (ConstructionPickEarly::FirstNonDeterioratingScore, 4, 1),
            (ConstructionPickEarly::FirstFeasibleScore, 5, 1),
            (
                ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard,
                3,
                0,
            ),
        ] {
            let mut c = settings(order, 100);
            c.construction.pick_early = pick;
            c.construction.assert_scores = true;
            let r = greedy_mixed_construction(&p, &n, c).unwrap();
            assert!(r.complete);
            assert_eq!(r.evaluations, eval);
            assert_eq!(r.assignment.basic().values(), &[Some(basic)]);
            assert!(!r.solution().unwrap().score.is_feasible());
            assert_eq!(r.termination, ConstructionTermination::Completed);
        }
    }
}
#[test]
fn impossible_components_and_shape_errors_precede_all_callbacks_even_at_zero_budget() {
    for order in ORDERS {
        for budget in [0, 1] {
            for p in [
                MixedProblem::new(
                    FiniteProblem::new(vec![Variable::<i32>::new(vec![])]).unwrap(),
                    ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap(),
                ),
                MixedProblem::new(
                    FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap(),
                    ListProblem::new(
                        vec![7],
                        vec![ListEntity::new(vec![]).with_value_range(vec![])],
                    )
                    .unwrap(),
                ),
            ] {
                let mut n = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
                n.add_constraint("never", vec![], |_| panic!()).unwrap();
                let r = greedy_mixed_construction(&p, &n, settings(order, budget)).unwrap();
                assert_eq!(r.termination, ConstructionTermination::NoCompleteAssignment);
                assert_eq!(r.evaluations, 0);
                assert!(r.score.is_none());
                assert!(!r.complete);
            }
            let p = problem();
            let wrong = MixedConstraintNetwork::<_, _, SimpleScore>::new(0, 2, 1);
            assert!(matches!(
                greedy_mixed_construction(&p, &wrong, settings(order, budget)),
                Err(MixedScoringError::ModelShape { .. })
            ));
        }
    }
}
#[test]
fn empty_and_already_complete_models_do_not_claim_an_unscored_solution() {
    for order in ORDERS {
        for p in [
            MixedProblem::new(
                FiniteProblem::<i32>::new(vec![]).unwrap(),
                ListProblem::<i32>::new(vec![], vec![]).unwrap(),
            ),
            MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap(),
                ListProblem::new(vec![7], vec![ListEntity::new(vec![7])]).unwrap(),
            ),
        ] {
            let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(
                p.basic().variables().len(),
                p.lists().entities().len(),
                p.lists().values().len(),
            );
            for budget in [0, 1] {
                let r = greedy_mixed_construction(&p, &n, settings(order, budget)).unwrap();
                assert!(r.complete);
                assert_eq!(r.evaluations, budget);
                assert_eq!(r.solution().is_some(), budget == 1);
                assert_eq!(r.assignment, p.initial_assignment());
                assert_eq!(
                    r.termination,
                    if budget == 0 {
                        ConstructionTermination::EvaluationLimit
                    } else {
                        ConstructionTermination::Completed
                    }
                );
            }
        }
    }
}
#[test]
fn later_errors_or_panics_do_not_mutate_the_model_after_partial_progress() {
    let p = problem();
    let original = p.initial_assignment();
    for order in ORDERS {
        for panic in [false, true] {
            let mut n = MixedConstraintNetwork::new(1, 2, 1);
            n.add_constraint(
                "fail second basic candidate",
                vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
                |f| {
                    if parts(f).0 == Some(1) {
                        if panic {
                            panic!("candidate failed")
                        };
                        return SimpleScore::ZERO.with_structural(1);
                    }
                    SimpleScore::ZERO
                },
            )
            .unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                greedy_mixed_construction(&p, &n, settings(order, 100))
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    Err(MixedScoringError::StructuralContribution { constraint: 0 })
                ));
            }
            assert_eq!(p.initial_assignment(), original);
        }
    }
}

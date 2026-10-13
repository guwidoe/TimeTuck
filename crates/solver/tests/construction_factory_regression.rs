//! Original inferred factories: actual solving, sorting/rebuilding and validation.
use std::rc::Rc;
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap()
}
fn bind(slots: Vec<usize>) -> ConstructionFieldBinding {
    ConstructionFieldBinding {
        name: "value".into(),
        slots,
    }
}
#[test]
fn all_inferred_modes_drive_real_configured_construction_and_fresh_copies() {
    let s = schema();
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().flatten().map(|&&v| i64::from(v)).sum())
    })
    .unwrap();
    for kind in [
        ConstructionPlacerKind::PooledEntity,
        ConstructionPlacerKind::QueuedEntity,
        ConstructionPlacerKind::QueuedValue,
    ] {
        let plan =
            ConstructionPlacerPlan::infer(&s, kind, None, &[], ConstructionEntitySort::Original)
                .unwrap();
        let recipe = ConstructionPlacerRecipe {
            plan,
            problem: p.clone(),
            bindings: vec![bind(vec![0, 1, 2])],
            sorter: None,
        };
        let mut original =
            RebuildablePlacer::build(recipe, ConstructionPlacerRecipe::build_provider).unwrap();
        let mut copy = original.copy().unwrap();
        let mut previous = None;
        for provider in [original.session_mut(), copy.session_mut()] {
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind_shared([0], provider).unwrap();
            let result = solve_with_extensions(
                &p,
                &n,
                SolverSequence {
                    phases: vec![SolverPhase::Construction(ConstructionConfig {
                        assert_scores: true,
                        ..Default::default()
                    })],
                    limits: SolverLimits::default(),
                },
                ext,
            )
            .unwrap();
            if kind == ConstructionPlacerKind::QueuedValue {
                assert_eq!(
                    result
                        .assignment
                        .values()
                        .iter()
                        .filter(|v| **v == Some(1))
                        .count(),
                    2
                );
                assert_eq!(
                    result
                        .assignment
                        .values()
                        .iter()
                        .filter(|v| **v == Some(2))
                        .count(),
                    1
                );
            } else {
                assert_eq!(result.assignment.values(), &[Some(2); 3]);
            }
            if let Some(prior) = previous.replace(result.assignment.clone()) {
                assert_eq!(prior, result.assignment);
            }
            assert_eq!(result.work.construction_steps, 3);
            assert_eq!(provider.queue().state(), SelectionCacheState::Idle);
        }
    }
}
#[test]
fn stable_sort_is_reopened_per_phase_and_absent_sorter_retains_encounter_order() {
    let s = schema();
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::QueuedEntity,
        None,
        &[],
        ConstructionEntitySort::DescendingIfAvailable,
    )
    .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 3]).unwrap();
    let a = p.initial_assignment();
    let bindings = vec![bind(vec![0, 1, 2])];
    for sorter in [
        None,
        Some(Rc::new(|x: usize, y: usize| y.cmp(&x)) as ConstructionEntityComparator),
    ] {
        let mut q = plan.build_queue(&p, &bindings, sorter.clone()).unwrap();
        q.event(SelectionLifecycleEvent::SolveStarted, &a).unwrap();
        for _ in 0..2 {
            q.event(SelectionLifecycleEvent::PhaseStarted, &a).unwrap();
            let mut keys = vec![];
            for _ in 0..3 {
                q.event(SelectionLifecycleEvent::StepStarted, &a).unwrap();
                keys.push(*q.next_placement(&a).unwrap().unwrap().key());
                q.event(SelectionLifecycleEvent::StepEnded, &a).unwrap();
            }
            assert_eq!(
                keys,
                if sorter.is_some() {
                    vec![2, 1, 0]
                } else {
                    vec![0, 1, 2]
                }
            );
            q.event(SelectionLifecycleEvent::PhaseEnded, &a).unwrap();
        }
        q.event(SelectionLifecycleEvent::SolveEnded, &a).unwrap();
    }
}
#[test]
fn inference_and_binding_reject_ambiguity_duplicates_and_invalid_slots() {
    let s = schema();
    assert!(matches!(
        ConstructionPlacerPlan::infer(
            &s,
            ConstructionPlacerKind::QueuedEntity,
            None,
            &["missing"],
            ConstructionEntitySort::Original
        ),
        Err(ConstructionFactoryError::Schema(_))
    ));
    assert!(matches!(
        ConstructionPlacerPlan::infer(
            &s,
            ConstructionPlacerKind::QueuedEntity,
            None,
            &["value", "value"],
            ConstructionEntitySort::Original
        ),
        Err(ConstructionFactoryError::Binding(_))
    ));
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::QueuedEntity,
        None,
        &[],
        ConstructionEntitySort::Original,
    )
    .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 2]).unwrap();
    for bindings in [
        vec![],
        vec![bind(vec![0, 0])],
        vec![bind(vec![2])],
        vec![bind(vec![0]), bind(vec![1])],
    ] {
        assert!(matches!(
            plan.build_queue(&p, &bindings, None),
            Err(ConstructionFactoryError::Binding(_))
        ));
    }
    let multi =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("a", false),
            PlanningVariableDefinition::basic::<i32>("b", false),
        ])])
        .unwrap();
    assert_eq!(
        ConstructionPlacerPlan::infer(
            &multi,
            ConstructionPlacerKind::QueuedValue,
            None,
            &[],
            ConstructionEntitySort::Original
        )
        .unwrap_err(),
        ConstructionFactoryError::AmbiguousField
    );
}

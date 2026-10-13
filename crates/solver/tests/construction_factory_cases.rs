// Complete pinned Community construction placer factory TESTS, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
struct Entity;
struct Multi;
struct Sortable;
fn schema<E: 'static>(names: &[&str]) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<E>(
        names
            .iter()
            .map(|&n| PlanningVariableDefinition::basic::<&str>(n, false))
            .collect(),
    )])
    .unwrap()
}
fn binding(name: &str, slots: Vec<usize>) -> ConstructionFieldBinding {
    ConstructionFieldBinding {
        name: name.into(),
        slots,
    }
}
fn open<V: Eq + Clone>(q: &mut ConstructionFactoryQueue<V>, a: &Assignment<V>) {
    q.event(SelectionLifecycleEvent::SolveStarted, a).unwrap();
    q.event(SelectionLifecycleEvent::PhaseStarted, a).unwrap();
    q.event(SelectionLifecycleEvent::StepStarted, a).unwrap();
}
#[test]
fn pooled_unfold_new() {
    let s = schema::<Entity>(&["value"]);
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::PooledEntity,
        None,
        &["value"],
        ConstructionEntitySort::Original,
    )
    .unwrap();
    assert_eq!(plan.changes.len(), 1);
    let change = &plan.changes[0];
    assert_eq!(change.explicit_entity, None);
    assert_eq!(change.entity_replay, Some(PlanningType::of::<Entity>()));
    assert_eq!(change.field.name(), "value");
    let p = FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"])]).unwrap();
    let mut q = plan
        .build_queue(&p, &[binding("value", vec![0])], None)
        .unwrap();
    let a = p.initial_assignment();
    open(&mut q, &a);
    assert_eq!(q.next_placement(&a).unwrap().unwrap().moves().len(), 2);
}
#[test]
fn queued_entity_build_from_unfold_new() {
    let s = schema::<Multi>(&["primaryValue", "secondaryValue"]);
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::QueuedEntity,
        None,
        &["primaryValue", "secondaryValue"],
        ConstructionEntitySort::Original,
    )
    .unwrap();
    assert_eq!(plan.entity, PlanningType::of::<Multi>());
    assert_eq!(plan.changes.len(), 2);
    assert_eq!(
        plan.changes
            .iter()
            .map(|c| c.field.name())
            .collect::<Vec<_>>(),
        vec!["primaryValue", "secondaryValue"]
    );
    let domain = vec!["e1v1", "e1v2", "e2v1", "e2v2"];
    let p = FiniteProblem::new(vec![
        Variable::new(domain.clone()).with_initial("e1v1"),
        Variable::new(domain.clone()).with_initial("e1v2"),
        Variable::new(domain.clone()).with_initial("e2v1"),
        Variable::new(domain).with_initial("e2v2"),
    ])
    .unwrap();
    let mut q = plan
        .build_queue(
            &p,
            &[
                binding("primaryValue", vec![0, 2]),
                binding("secondaryValue", vec![1, 3]),
            ],
            None,
        )
        .unwrap();
    let a = p.initial_assignment();
    open(&mut q, &a);
    assert!(q.has_next().unwrap());
    let placement = q.next_placement(&a).unwrap().unwrap();
    assert_eq!(*placement.key(), 0);
    let mut it = placement.iter();
    for value in ["e1v1", "e1v2", "e2v1", "e2v2"] {
        assert_eq!(
            it.next(),
            Some(&Move::Change {
                variable: 0,
                value: Some(value)
            })
        );
    }
    assert!(it.next().is_none());
}
#[test]
fn queued_entity_build_with_entity_sort_manner() {
    let s = schema::<Sortable>(&["primaryValue"]);
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::QueuedEntity,
        None,
        &["primaryValue"],
        ConstructionEntitySort::DescendingIfAvailable,
    )
    .unwrap();
    assert_eq!(plan.sort, ConstructionEntitySort::DescendingIfAvailable);
    assert_eq!(plan.entity_order(), ConstructionEntityOrder::Sorted);
    let p = FiniteProblem::new(vec![Variable::new(vec!["v"]); 2]).unwrap();
    let sorter: ConstructionEntityComparator = std::rc::Rc::new(|a, b| b.cmp(&a));
    let mut q = plan
        .build_queue(&p, &[binding("primaryValue", vec![0, 1])], Some(sorter))
        .unwrap();
    let a = p.initial_assignment();
    open(&mut q, &a);
    assert_eq!(*q.next_placement(&a).unwrap().unwrap().key(), 1);
}
#[test]
fn queued_value_build_without_configured_move_selector() {
    let s = schema::<Entity>(&["value"]);
    let plan = ConstructionPlacerPlan::infer(
        &s,
        ConstructionPlacerKind::QueuedValue,
        Some(PlanningType::of::<Entity>()),
        &[],
        ConstructionEntitySort::Original,
    )
    .unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert!(plan.changes[0].value_replay);
    let p = FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"]); 2]).unwrap();
    let mut q = plan
        .build_queue(&p, &[binding("value", vec![0, 1])], None)
        .unwrap();
    let a = p.initial_assignment();
    open(&mut q, &a);
    assert!(q.has_next().unwrap());
    let placement = q.next_placement(&a).unwrap().unwrap();
    let mut it = placement.iter();
    for variable in [0, 1] {
        assert_eq!(
            it.next(),
            Some(&Move::Change {
                variable,
                value: Some("v1")
            })
        );
    }
    assert!(it.next().is_none());
}

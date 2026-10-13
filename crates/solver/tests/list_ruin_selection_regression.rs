//! Original identity, callback safety and executable reconstruction controls.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct World;
struct Job;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Job>(vec![
        PlanningVariableDefinition::list::<i32>("route", false),
        PlanningVariableDefinition::basic::<i32>("worker", false),
    ])])
    .unwrap()
}
#[derive(Clone, Debug)]
struct Handle(Rc<i32>);
impl PartialEq for Handle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Handle {}
#[test]
fn metadata_preserves_value_order_entity_set_and_exact_lookup_results() {
    let s = schema();
    let v = s
        .genuine_entity::<Job>()
        .unwrap()
        .genuine_variable("route")
        .unwrap();
    let a = Handle(Rc::new(1));
    let same_payload = Handle(Rc::new(1));
    let b = Handle(Rc::new(2));
    let movement = ListRuinSelection::new(
        v,
        vec![a.clone(), b.clone(), a.clone()],
        vec![a.clone(), b.clone(), a.clone()],
    )
    .unwrap();
    assert_eq!(movement.planning_entities(), &[a.clone(), b.clone()]);
    assert_eq!(
        movement,
        ListRuinSelection::new(
            v,
            vec![a.clone(), b.clone(), a.clone()],
            vec![b.clone(), a.clone()]
        )
        .unwrap()
    );
    assert_ne!(
        movement,
        ListRuinSelection::new(
            v,
            vec![b.clone(), a.clone(), a.clone()],
            vec![a.clone(), b.clone()]
        )
        .unwrap()
    );
    assert_ne!(
        movement,
        ListRuinSelection::new(
            v,
            vec![same_payload, b.clone(), a.clone()],
            vec![a.clone(), b.clone()]
        )
        .unwrap()
    );
    let calls = RefCell::new(vec![]);
    let destination = Handle(Rc::new(7));
    let rebased = movement
        .rebase(
            |_| {
                calls.borrow_mut().push("value");
                Ok::<_, ()>(destination.clone())
            },
            |_| {
                calls.borrow_mut().push("entity");
                Ok(destination.clone())
            },
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        ["value", "value", "value", "entity", "entity"]
    );
    assert_eq!(rebased.planning_values().len(), 3);
    assert_eq!(rebased.planning_entities().len(), 1);
    assert!(
        rebased
            .planning_values()
            .iter()
            .all(|v| Rc::ptr_eq(&v.0, &destination.0))
    );
    assert!(Rc::ptr_eq(
        &rebased.planning_entities()[0].0,
        &destination.0
    ));
    assert_eq!(movement.planning_values(), &[a.clone(), b.clone(), a]);
}
#[test]
fn errors_panics_and_invalid_metadata_preserve_selection_and_stop_callbacks() {
    let s = schema();
    let e = s.genuine_entity::<Job>().unwrap();
    let v = e.genuine_variable("route").unwrap();
    assert!(matches!(
        ListRuinSelection::new(e.genuine_variable("worker").unwrap(), vec![1], vec![0]),
        Err(ListRuinSelectionError::NotList { .. })
    ));
    let selection = ListRuinSelection::new(v, vec![1, 2, 3], vec![0, 1]).unwrap();
    let calls = RefCell::new(vec![]);
    let failed = selection.rebase(
        |v| {
            calls.borrow_mut().push(*v);
            if *v == 2 { Err("fail") } else { Ok(*v) }
        },
        |_| -> Result<i32, &str> { panic!("entity called after value error") },
    );
    assert!(matches!(
        failed,
        Err(ListRuinSelectionError::Lookup("fail"))
    ));
    assert_eq!(*calls.borrow(), [1, 2]);
    calls.borrow_mut().clear();
    assert!(
        catch_unwind(AssertUnwindSafe(|| selection.rebase(
            |v| {
                calls.borrow_mut().push(*v);
                Ok::<_, ()>(*v)
            },
            |entity| -> Result<i32, ()> {
                calls.borrow_mut().push(100 + *entity);
                panic!("entity callback")
            }
        )))
        .is_err()
    );
    assert_eq!(*calls.borrow(), [1, 2, 3, 100]);
    assert_eq!(selection.planning_values(), [1, 2, 3]);
    assert_eq!(selection.planning_entities(), [0, 1]);
    assert_eq!(
        selection.rebase(|v| Ok::<_, ()>(*v), |e| Ok(*e)).unwrap(),
        selection
    );
}
#[test]
fn rebased_selection_prepares_real_repair_move_with_pins_ranges_and_undo() {
    let s = schema();
    let descriptor = s
        .genuine_entity::<Job>()
        .unwrap()
        .genuine_variable("route")
        .unwrap();
    // Identity payloads may be arbitrary handles; these mappings bind them to the
    // actual target registry and entity indices after rebasing.
    let selection =
        ListRuinSelection::new(descriptor, vec!["one", "three"], vec!["owner"]).unwrap();
    let rebased = selection
        .rebase(
            |v| Ok::<_, ()>(if *v == "one" { 1usize } else { 3 }),
            |_| Ok(0usize),
        )
        .unwrap();
    let problem = ListProblem::new_indexed(
        vec![0, 1, 2, 3, 4],
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![4]).with_pinned_prefix(1),
        ],
    )
    .unwrap();
    let before = problem.initial_assignment();
    let plan = rebased
        .prepare(&problem, &before, |v| Ok::<_, ()>(*v), |e| Ok(*e))
        .unwrap();
    assert_eq!(
        plan.repair_problem().initial_assignment().lists(),
        &[vec![0, 2], vec![4]]
    );
    let rebuilt = ListAssignment::new(vec![vec![0, 2], vec![4, 3, 1]]);
    let movement = plan.finish(&rebuilt).unwrap();
    let mut actual = before.clone();
    {
        let trial = problem.apply(&mut actual, &movement).unwrap();
        assert_eq!(&*trial, &rebuilt);
    }
    assert_eq!(actual, before);
    problem.apply(&mut actual, &movement).unwrap().commit();
    assert_eq!(actual, rebuilt);
    assert!(matches!(
        plan.finish(&ListAssignment::new(vec![vec![0, 1, 2, 3], vec![]])),
        Err(ListRuinPlanError::Model(_))
    ));
    let unknown = ListRuinSelection::new(descriptor, vec![1usize], vec![99usize]).unwrap();
    assert!(matches!(
        unknown.prepare(&problem, &before, |v| Ok::<_, ()>(*v), |e| Ok(*e)),
        Err(ListRuinSelectionError::UnknownEntity { .. })
    ));
    let absent = ListRuinSelection::new(descriptor, vec![1usize], vec![1usize]).unwrap();
    assert!(matches!(
        absent.prepare(&problem, &before, |v| Ok::<_, ()>(*v), |e| Ok(*e)),
        Err(ListRuinSelectionError::UnlistedOwner { entity: 0 })
    ));
    let pinned = ListRuinSelection::new(descriptor, vec![0usize], vec![0usize]).unwrap();
    assert!(matches!(
        pinned.prepare(&problem, &before, |v| Ok::<_, ()>(*v), |e| Ok(*e)),
        Err(ListRuinSelectionError::Plan(
            ListRuinPlanError::PinnedValue { value: 0 }
        ))
    ));
    let duplicate = ListRuinSelection::new(descriptor, vec![1usize, 1], vec![0usize]).unwrap();
    assert!(matches!(
        duplicate.prepare(&problem, &before, |v| Ok::<_, ()>(*v), |e| Ok(*e)),
        Err(ListRuinSelectionError::Plan(
            ListRuinPlanError::DuplicateValue { value: 1 }
        ))
    ));
    assert_eq!(before, problem.initial_assignment());
}

//! Original selection identity, destination repair execution and failure controls.
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", true),
        PlanningVariableDefinition::list::<i32>("list", true),
    ])])
    .unwrap()
}
#[test]
fn footprint_sets_and_entity_order_are_independent_of_generated_writes() {
    let s = schema();
    let v = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let a = BasicRuinSelection::new(v, vec![1, 0], vec![7, 8, 7]).unwrap();
    let same = BasicRuinSelection::new(v, vec![1, 0], vec![8, 7]).unwrap();
    assert_eq!(a, same);
    assert_eq!(a.planning_values(), &[7, 8]);
    assert_ne!(
        a,
        BasicRuinSelection::new(v, vec![0, 1], vec![7, 8]).unwrap()
    );
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).allowing_unassigned(),
    ])
    .unwrap();
    let before = p.initial_assignment();
    let plan = a.prepare(&p, &before).unwrap();
    let rebuilt = Assignment::new(vec![Some(1), Some(1)]);
    let movement = plan.finish(&rebuilt).unwrap();
    let mut actual = before.clone();
    {
        let trial = p.apply(&mut actual, &movement).unwrap();
        assert_eq!(*trial, rebuilt);
    }
    assert_eq!(actual, before);
    assert_eq!(a, same);
    assert_ne!(movement, plan.finish(&before).unwrap());
    let rebased = a.rebase(|i| Ok::<_, ()>(1 - i), |x| Ok(x + 10)).unwrap();
    assert_eq!(rebased.planning_entities(), &[0, 1]);
    let repair = rebased.prepare(&p, &before).unwrap();
    let m = repair.finish(&rebuilt).unwrap();
    p.apply(&mut actual, &m).unwrap().commit();
    assert_eq!(actual, rebuilt);
}
#[test]
fn checked_metadata_and_rebase_errors_preserve_reusable_selection() {
    let s = schema();
    let e = s.genuine_entity::<Entity>().unwrap();
    let v = e.genuine_variable("value").unwrap();
    assert_eq!(
        BasicRuinSelection::<i32>::new(e.genuine_variable("list").unwrap(), vec![0], [])
            .unwrap_err(),
        BasicRuinSelectionError::NotBasic
    );
    assert_eq!(
        BasicRuinSelection::<i32>::new(v, vec![0, 0], []).unwrap_err(),
        BasicRuinSelectionError::DuplicateEntity { entity: 0 }
    );
    let a = BasicRuinSelection::new(v, vec![0, 1], vec![4, 5]).unwrap();
    assert_eq!(
        a.rebase(|_| Ok::<_, ()>(0), |x| Ok(*x)).unwrap_err(),
        MoveRebaseError::Collision(MoveRebaseNamespace::BasicVariable)
    );
    assert_eq!(
        a.rebase(Ok::<_, ()>, |_| Ok(4)).unwrap_err(),
        MoveRebaseError::Collision(MoveRebaseNamespace::BasicValue)
    );
    assert_eq!(
        a.rebase(Ok::<_, &str>, |_| Err::<i32, _>("missing"))
            .unwrap_err(),
        MoveRebaseError::Lookup("missing")
    );
    assert_eq!(a.planning_entities(), &[0, 1]);
    assert_eq!(a.planning_values(), &[4, 5]);
    assert_eq!(a, a.rebase(Ok::<_, ()>, |x| Ok(*x)).unwrap());
}

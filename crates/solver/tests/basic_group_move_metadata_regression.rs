//! Original semantic identity, metadata validation and real execution controls.
use timetuck::*;
struct World;
struct Job;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Job>(vec![
        PlanningVariableDefinition::basic::<i32>("primary", false),
        PlanningVariableDefinition::basic::<i32>("secondary", false),
        PlanningVariableDefinition::shadow::<i32>("shadow"),
    ])])
    .unwrap()
}
#[test]
fn semantic_identity_survives_snapshot_changes_and_retains_orientation() {
    let s = schema();
    let e = s.genuine_entity::<Job>().unwrap();
    let v = e.genuine_variable("primary").unwrap();
    let left = Pillar::new([0, 1]).unwrap();
    let right = Pillar::new([2]).unwrap();
    let a = PillarSwapPlan::new(vec![v], left.clone(), right.clone()).unwrap();
    let reverse = PillarSwapPlan::new(vec![v], right, left).unwrap();
    assert_ne!(a, reverse);
    let mut state = Assignment::new(vec![Some(1), Some(1), Some(2)]);
    let first = a.movement(&state).unwrap();
    let same = a.clone();
    {
        let trial = state.apply(&first).unwrap();
        assert_eq!(trial.values(), &[Some(2), Some(2), Some(1)]);
        trial.commit();
    }
    assert_eq!(a, same);
    assert_ne!(first, a.movement(&state).unwrap());
    // Identical values produce identical write snapshots despite different identity.
    let equal = Assignment::new(vec![Some(9); 3]);
    let other = PillarSwapPlan::new(
        vec![e.genuine_variable("secondary").unwrap()],
        a.left().clone(),
        a.right().clone(),
    )
    .unwrap();
    assert_ne!(a, other);
    assert_eq!(a.movement(&equal).unwrap(), other.movement(&equal).unwrap());
    let copy_schema = schema();
    let copy = copy_schema
        .genuine_entity::<Job>()
        .unwrap()
        .genuine_variable("primary")
        .unwrap();
    assert_ne!(
        a,
        PillarSwapPlan::new(vec![copy], a.left().clone(), a.right().clone()).unwrap()
    );
}
#[test]
fn multicolumn_execution_and_descriptions_use_registered_metadata() {
    let s = schema();
    let e = s.genuine_entity::<Job>().unwrap();
    let p = e.genuine_variable("primary").unwrap();
    let q = e.genuine_variable("secondary").unwrap();
    let a = PillarSwapPlan::new(
        vec![p, q],
        Pillar::from_rows(vec![vec![0, 1], vec![2, 3]]).unwrap(),
        Pillar::from_rows(vec![vec![4, 5]]).unwrap(),
    )
    .unwrap();
    assert_eq!(a.variables(), &[p, q]);
    assert_eq!(
        a.describe(None),
        format!(
            "PillarSwapMove({}.primary, {}.secondary)",
            std::any::type_name::<Job>(),
            std::any::type_name::<Job>()
        )
    );
    let mut state = Assignment::new(vec![Some(1), Some(2), Some(1), Some(2), Some(3), Some(4)]);
    let movement = a.movement(&state).unwrap();
    {
        let trial = state.apply(&movement).unwrap();
        assert_eq!(
            trial.values(),
            &[Some(3), Some(4), Some(3), Some(4), Some(1), Some(2)]
        );
    }
    assert_eq!(
        state.values(),
        &[Some(1), Some(2), Some(1), Some(2), Some(3), Some(4)]
    );
    let mass = NamedMassChange::new(q, mass_change_from_members([1, 3, 5], Some(7))).unwrap();
    {
        let trial = state.apply(mass.movement()).unwrap();
        assert_eq!(
            trial.values(),
            &[Some(1), Some(7), Some(1), Some(7), Some(3), Some(7)]
        );
        trial.commit();
    }
    assert_eq!(
        mass.describe(Some("Task")),
        "MassChangeMove(Task.secondary)"
    );
    let distinct = NamedMassChange::new(p, mass.change().clone()).unwrap();
    assert_ne!(mass, distinct);
    assert_eq!(mass.movement(), distinct.movement());
}
#[test]
fn invalid_metadata_and_nonuniform_current_state_fail_without_mutation() {
    let s = schema();
    let e = s.genuine_entity::<Job>().unwrap();
    let p = e.genuine_variable("primary").unwrap();
    let shadow = e.shadow_variable("shadow").unwrap();
    assert!(NamedMassChange::new(shadow, mass_change_from_members([0], Some(1))).is_err());
    assert!(
        PillarSwapPlan::new(
            vec![p, p],
            Pillar::from_rows(vec![vec![0, 1]]).unwrap(),
            Pillar::from_rows(vec![vec![2, 3]]).unwrap()
        )
        .is_err()
    );
    assert!(
        PillarSwapPlan::new(
            vec![p],
            Pillar::new([0]).unwrap(),
            Pillar::new([0]).unwrap()
        )
        .is_err()
    );
    assert!(
        PillarSwapPlan::new(vec![], Pillar::new([0]).unwrap(), Pillar::new([1]).unwrap()).is_err()
    );
    let a = PillarSwapPlan::new(
        vec![p],
        Pillar::new([0, 1]).unwrap(),
        Pillar::new([2]).unwrap(),
    )
    .unwrap();
    let state = Assignment::new(vec![Some(1), Some(2), Some(3)]);
    assert!(matches!(
        a.movement(&state),
        Err(PillarError::NotUniform { .. })
    ));
    assert_eq!(state.values(), &[Some(1), Some(2), Some(3)]);
}

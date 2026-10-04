// Adapted from pinned Community MassChangeMoveProviderTest, Apache-2.0.
// Fixtures and API/enumeration substitutions: compat/provenance.json.
use timetuck::*;
fn generated(n: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..n)
            .map(|i| Variable::new(vec![0, 1]).with_initial((i % 2) as i32))
            .collect(),
    )
    .unwrap()
}
fn moves(p: &FiniteProblem<i32>, c: MassChangeConfig, limit: usize) -> Vec<MassChange<i32>> {
    mass_change_moves(p, &p.initial_assignment(), c)
        .unwrap()
        .take(limit)
        .collect()
}
#[test]
fn mixed_value_sample_yields_one_move_setting_every_member_to_a_legal_value() {
    let p = generated(2);
    assert_ne!(p.variables()[0].initial(), p.variables()[1].initial());
    let m = moves(&p, MassChangeConfig::exactly(2), 50);
    assert!(!m.is_empty());
    for v in m {
        assert!(v.members().iter().all(|i| [0, 1].contains(i)));
        assert!([Some(&0), Some(&1)].contains(&v.destination()));
    }
}
#[test]
fn assign_side_disabled_excludes_unassigned_entities() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![0, 1])
            .with_initial(1)
            .allowing_unassigned(),
    ])
    .unwrap();
    let m = moves(&p, MassChangeConfig::all().with_crossing_null(false), 50);
    assert!(!m.is_empty());
    for v in m {
        assert!(!v.members().contains(&0));
        assert!(v.destination().is_some());
    }
}
#[test]
fn mixed_sample_may_assign_or_unassign() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![0]).with_initial(0).allowing_unassigned(),
    ])
    .unwrap();
    let m = moves(&p, MassChangeConfig::all(), 50);
    assert!(!m.is_empty());
    for v in &m {
        assert_eq!(v.members(), [0, 1]);
        if let Some(value) = v.destination() {
            assert_eq!(*value, 0);
        }
    }
    assert!(m.iter().any(|v| v.destination().is_none()));
    assert!(m.iter().any(|v| v.destination().is_some()));
}
#[test]
fn crossing_null_default_false_when_variable_does_not_allow_unassigned() {
    let p = generated(2);
    assert_ne!(p.variables()[0].initial(), p.variables()[1].initial());
    assert!(!moves(&p, MassChangeConfig::exactly(2), 50).is_empty());
}
#[test]
fn constructor_rejects_explicit_crossing_null_on_non_unassigned_variable() {
    let p = generated(2);
    assert!(matches!(
        mass_change_moves(
            &p,
            &p.initial_assignment(),
            MassChangeConfig::exactly(2).with_crossing_null(true)
        ),
        Err(ModelError::NullCrossingRequired { variable: 0 })
    ));
}
#[test]
fn homogeneous_sample_never_produces_a_no_op_move() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 5]).unwrap();
    let m = moves(&p, MassChangeConfig::exactly(2), 200);
    assert!(!m.is_empty());
    assert!(m.iter().all(|v| v.destination() != Some(&0)));
}
#[test]
fn size_one_sample_produces_no_move() {
    assert!(moves(&generated(5), MassChangeConfig::exactly(1), 10).is_empty());
}
fn disjoint() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![2, 3])
            .with_initial(2)
            .allowing_unassigned(),
    ])
    .unwrap()
}
#[test]
fn empty_intersection_ends_the_iterator_instead_of_hanging() {
    assert!(
        moves(
            &disjoint(),
            MassChangeConfig::all().with_crossing_null(false),
            10
        )
        .is_empty()
    );
}
#[test]
fn empty_intersection_still_yields_null_destination_when_crossing_null() {
    let m = moves(&disjoint(), MassChangeConfig::all(), 10);
    assert!(!m.is_empty());
    assert!(m.iter().all(|v| v.destination().is_none()));
}
#[test]
fn pinned_entity_sharing_a_value_with_free_entities_never_joins_a_sample() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let m = moves(&p, MassChangeConfig::exactly(2), 50);
    assert!(!m.is_empty());
    assert!(m.iter().all(|v| !v.members().contains(&0)));
}

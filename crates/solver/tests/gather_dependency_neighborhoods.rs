//! Original dependency-aware neighborhood regression.
use timetuck::*;
#[test]
fn invalid_dependency_gathers_are_skipped() {
    let p = ListProblem::new_indexed(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    let a = p.initial_assignment();
    assert_eq!(list_gather_moves(&p, &a, 1).unwrap().count(), 0);
}

fn valid(
    p: &ListProblem<usize>,
    base: &ListProblem<usize>,
    a: &ListAssignment<usize>,
    m: &ListMove<usize>,
) -> bool {
    // Materialize with the dependency-free model, then independently validate
    // the complete final assignment against the constrained model.
    let mut next = a.clone();
    base.apply(&mut next, m).unwrap().commit();
    p.validate(&next).is_ok()
}

#[test]
fn constrained_neighborhoods_keep_valid_candidate_order_and_mass_metadata() {
    for indexed in [false, true] {
        for pinned in [false, true] {
            let values = vec![6, 4, 0, 5, 1, 3, 2];
            let entities = vec![
                ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(usize::from(pinned)),
                ListEntity::new(vec![3]).with_value_range(vec![0, 1, 2, 3, 4]),
                ListEntity::new(vec![6]).with_pinned(pinned),
            ];
            let base = if indexed {
                ListProblem::new_indexed(values.clone(), entities)
            } else {
                ListProblem::new(values.clone(), entities)
            }
            .unwrap()
            .allowing_unassigned();
            let dependencies = values
                .iter()
                .map(|&id| {
                    if id == 0 {
                        vec![]
                    } else {
                        vec![values.iter().position(|&v| v == id - 1).unwrap()]
                    }
                })
                .collect();
            let p = base.clone().with_dependencies(dependencies).unwrap();
            let a = p.initial_assignment();
            let size = ListSampleSize::exactly(2);
            let config = ListMassChangeConfig::new(size);
            for family in [
                Neighborhood::Gather { max_size: 2 },
                Neighborhood::MassListChange(config),
                Neighborhood::MassListAssign(size),
                Neighborhood::MassListUnassign(size),
            ] {
                let all: Vec<_> = list_moves(&base, &a, family.clone()).unwrap().collect();
                let expected: Vec<_> = all
                    .iter()
                    .filter(|m| valid(&p, &base, &a, m))
                    .cloned()
                    .collect();
                let actual: Vec<_> = list_moves(&p, &a, family.clone()).unwrap().collect();
                assert_eq!(actual, expected, "{family:?}");
                assert!(!actual.is_empty());
                if !matches!(family, Neighborhood::MassListUnassign(_)) {
                    assert!(actual.len() < all.len());
                }
                assert_eq!(
                    list_moves(&p, &a, family).unwrap().next(),
                    actual.first().cloned()
                );
                for m in actual {
                    let mut next = a.clone();
                    {
                        let trial = p.apply(&mut next, &m).unwrap();
                        p.validate(&trial).unwrap();
                    }
                    assert_eq!(next, a);
                }
            }
            let expected: Vec<_> = list_mass_change_moves(&base, &a, config)
                .unwrap()
                .filter(|m| valid(&p, &base, &a, m.movement()))
                .collect();
            assert_eq!(
                list_mass_change_moves(&p, &a, config)
                    .unwrap()
                    .collect::<Vec<_>>(),
                expected
            );
            let expected: Vec<_> = list_mass_assign_moves(&base, &a, size)
                .unwrap()
                .filter(|m| valid(&p, &base, &a, m.movement()))
                .collect();
            assert_eq!(
                list_mass_assign_moves(&p, &a, size)
                    .unwrap()
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                sample_list_mass_unassign_moves(&p, &a, size, 918, 64)
                    .unwrap()
                    .collect::<Vec<_>>(),
                sample_list_mass_unassign_moves(&base, &a, size, 918, 64)
                    .unwrap()
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn source_validation_precedes_configuration_even_for_empty_work() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let invalid = ListAssignment::new(vec![vec![0, 0]]);
    let expected = p.validate(&invalid).unwrap_err();
    let config = ListMassChangeConfig::new(ListSampleSize::exactly(0)).with_unassignment(true);
    assert_eq!(
        list_gather_moves(&p, &invalid, 0).err(),
        Some(expected.clone())
    );
    assert_eq!(
        list_mass_change_moves(&p, &invalid, config).err(),
        Some(expected.clone())
    );
    assert_eq!(
        list_mass_assign_moves(&p, &invalid, ListSampleSize::exactly(0)).err(),
        Some(expected.clone())
    );
    assert_eq!(
        sample_list_mass_unassign_moves(&p, &invalid, ListSampleSize::exactly(0), 0, 0).err(),
        Some(expected)
    );
    let a = p.initial_assignment();
    assert_eq!(
        list_mass_change_moves(&p, &a, config).err(),
        Some(ListError::UnassignmentNotAllowed)
    );
    assert_eq!(
        list_mass_assign_moves(&p, &a, ListSampleSize::exactly(0)).err(),
        Some(ListError::OptionalValuesRequired)
    );
}

#[test]
fn sampled_unassignment_iterator_retains_thread_safety() {
    fn require_send<T: Send + Sync>(value: T) -> T {
        value
    }
    let p = ListProblem::new_indexed(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let moves = require_send(
        sample_list_mass_unassign_moves(&p, &a, ListSampleSize::exactly(1), 37, 4).unwrap(),
    );
    assert_eq!(moves.count(), 4);
}

// Adapted from pinned Community MassListAssign/UnassignMoveProviderTest, Apache-2.0.
// Exact methods, fixtures and substitutions are recorded in compat/provenance.json.
use timetuck::*;
mod assign {
    use super::*;
    fn moves(p: &ListProblem<i32>, size: ListSampleSize, limit: usize) -> Vec<ListMassChange<i32>> {
        list_mass_assign_moves(p, &p.initial_assignment(), size)
            .unwrap()
            .take(limit)
            .collect()
    }
    #[test]
    fn constructor_rejects_non_unassigned_variable() {
        let p = ListProblem::<i32>::new(vec![], vec![]).unwrap();
        assert!(matches!(
            list_mass_assign_moves(&p, &p.initial_assignment(), ListSampleSize::exactly(2)),
            Err(ListError::OptionalValuesRequired)
        ));
    }
    #[test]
    fn sample_members_are_always_a_subset_of_the_unassigned_values() {
        let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0])])
            .unwrap()
            .allowing_unassigned();
        let m = moves(&p, ListSampleSize::exactly(2), 50);
        assert!(!m.is_empty());
        for v in m {
            assert!(v.destination().is_some());
            assert!(v.members().iter().all(|v| [1, 2].contains(v)));
            assert!(!v.members().contains(&0));
        }
    }
    #[test]
    fn destination_respects_every_members_value_range() {
        let p = ListProblem::new(
            vec![0, 1],
            vec![
                ListEntity::new(vec![]).with_value_range(vec![0, 1]),
                ListEntity::new(vec![]).with_value_range(vec![0]),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let m = moves(&p, ListSampleSize::all(), 50);
        assert!(!m.is_empty());
        for v in m {
            let at = v.destination().unwrap();
            assert!(
                v.members()
                    .iter()
                    .all(|x| p.entities()[at.entity].value_range().unwrap().contains(x))
            );
        }
    }
    #[test]
    fn destination_never_falls_in_the_pinned_prefix() {
        let p = ListProblem::new(
            (0..5).collect(),
            vec![ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(2)],
        )
        .unwrap()
        .allowing_unassigned();
        let m = moves(&p, ListSampleSize::exactly(2), 100);
        assert!(!m.is_empty());
        for v in m {
            assert!(v.destination().unwrap().index >= 2);
        }
    }
    #[test]
    fn fully_pinned_entity_is_never_assigned_into() {
        let p = ListProblem::new(
            vec![0, 1, 2],
            vec![
                ListEntity::new(vec![0]).with_pinned(true),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let m = moves(&p, ListSampleSize::exactly(2), 100);
        assert!(!m.is_empty());
        for v in m {
            assert_ne!(v.destination().unwrap().entity, 0);
        }
    }
}
mod unassign {
    use super::*;
    fn moves(p: &ListProblem<i32>, size: ListSampleSize, limit: usize) -> Vec<ListMassChange<i32>> {
        list_mass_unassign_moves(p, &p.initial_assignment(), size)
            .unwrap()
            .take(limit)
            .collect()
    }
    fn eight() -> ListProblem<i32> {
        ListProblem::new((0..8).collect(), vec![ListEntity::new((0..8).collect())])
            .unwrap()
            .allowing_unassigned()
    }
    #[test]
    fn constructor_rejects_non_unassigned_variable() {
        let p = ListProblem::<i32>::new(vec![], vec![]).unwrap();
        assert!(matches!(
            list_mass_unassign_moves(&p, &p.initial_assignment(), ListSampleSize::exactly(2)),
            Err(ListError::OptionalValuesRequired)
        ));
    }
    #[test]
    fn every_move_has_a_null_destination() {
        let m = moves(&eight(), ListSampleSize::exactly(3), 50);
        assert!(!m.is_empty());
        assert!(m.iter().all(|m| m.destination().is_none()));
    }
    #[test]
    fn size_one_sample_still_yields_a_move_and_the_provider_is_not_cut_off_early() {
        let p = eight();
        let a = p.initial_assignment();
        let m: Vec<_> = sample_list_mass_unassign_moves(&p, &a, ListSampleSize::exactly(1), 31, 50)
            .unwrap()
            .collect();
        assert_eq!(m.len(), 50);
    }
    #[test]
    fn pinned_value_never_joins_a_sample() {
        let p = ListProblem::new(
            (0..4).collect(),
            vec![ListEntity::new((0..4).collect()).with_pinned_prefix(2)],
        )
        .unwrap()
        .allowing_unassigned();
        let m = moves(&p, ListSampleSize::exactly(2), 50);
        assert!(!m.is_empty());
        assert!(
            m.iter()
                .all(|m| !m.members().contains(&0) && !m.members().contains(&1))
        );
    }
    #[test]
    fn fully_pinned_entity_contributes_no_members() {
        let p = ListProblem::new(
            (0..5).collect(),
            vec![
                ListEntity::new(vec![0, 1]).with_pinned(true),
                ListEntity::new(vec![2, 3, 4]),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let m = moves(&p, ListSampleSize::exactly(2), 50);
        assert!(!m.is_empty());
        assert!(
            m.iter()
                .all(|m| !m.members().contains(&0) && !m.members().contains(&1))
        );
    }
}

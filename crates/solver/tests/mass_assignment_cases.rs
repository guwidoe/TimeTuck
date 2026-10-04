// Adapted from Community MassAssign/UnassignMoveProviderTest, Apache-2.0.
// Source identities, fixture notices and API substitutions: compat/provenance.json.
use timetuck::*;
fn missing(n: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0]).allowing_unassigned(); n]).unwrap()
}
mod assign {
    use super::*;
    #[test]
    fn sample_members_are_always_a_subset_of_the_unassigned_entities() {
        let p = missing(5);
        let a = p.initial_assignment();
        let m: Vec<_> = mass_assign_moves(&p, &a, MassAssignmentSize::exactly(2))
            .unwrap()
            .take(200)
            .collect();
        assert!(!m.is_empty());
        for v in m {
            assert!(v.members().len() <= 2);
            assert!(v.members().iter().all(|i| (0..5).contains(i)));
            assert_eq!(v.destination(), Some(&0));
        }
    }
    #[test]
    fn different_draws_produce_different_samples() {
        let p = missing(5);
        let a = p.initial_assignment();
        let groups: std::collections::BTreeSet<_> =
            mass_assign_moves(&p, &a, MassAssignmentSize::exactly(2))
                .unwrap()
                .take(200)
                .map(|m| {
                    m.members()
                        .iter()
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .collect();
        assert!(groups.len() > 1);
    }
    #[test]
    fn pinned_unassigned_entity_never_joins_a_sample() {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0])
                .allowing_unassigned()
                .with_pinned(true),
            Variable::new(vec![0]).allowing_unassigned(),
            Variable::new(vec![0]).allowing_unassigned(),
        ])
        .unwrap();
        let a = p.initial_assignment();
        let m: Vec<_> = mass_assign_moves(&p, &a, MassAssignmentSize::exactly(2))
            .unwrap()
            .take(50)
            .collect();
        assert!(!m.is_empty());
        assert!(m.iter().all(|m| !m.members().contains(&0)));
    }
}
mod unassign {
    use super::*;
    #[test]
    fn constructor_rejects_non_unassigned_variable() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
        assert!(matches!(
            mass_unassign_moves(&p, &p.initial_assignment(), MassAssignmentSize::exactly(2)),
            Err(ModelError::NullCrossingRequired { variable: 0 })
        ));
    }
    #[test]
    fn mixed_value_sample_yields_one_move_nulling_every_member() {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .with_initial(0)
                .allowing_unassigned(),
            Variable::new(vec![0, 1])
                .with_initial(1)
                .allowing_unassigned(),
            Variable::new(vec![0, 1]).allowing_unassigned(),
        ])
        .unwrap();
        let a = p.initial_assignment();
        let m: Vec<_> = mass_unassign_moves(&p, &a, MassAssignmentSize::exactly(2))
            .unwrap()
            .take(50)
            .collect();
        assert!(!m.is_empty());
        for v in m {
            assert!(!v.members().contains(&2));
            assert!(v.members().iter().all(|i| [0, 1].contains(i)));
            assert!(v.destination().is_none());
        }
    }
    #[test]
    fn size_one_sample_still_yields_a_move_and_the_provider_is_not_cut_off_early() {
        let p = FiniteProblem::new(
            (0..5)
                .map(|i| {
                    let v = Variable::new(vec![0, 1, 2]).allowing_unassigned();
                    if i == 0 { v } else { v.with_initial(i % 3) }
                })
                .collect(),
        )
        .unwrap();
        let a = p.initial_assignment();
        let m: Vec<_> = sample_mass_unassign_moves(&p, &a, MassAssignmentSize::exactly(1), 29, 50)
            .unwrap()
            .collect();
        assert_eq!(m.len(), 50);
        for v in m {
            assert_eq!(v.members().len(), 1);
            assert!(v.destination().is_none());
        }
    }
    #[test]
    fn pinned_entity_never_joins_a_sample() {
        let v = Variable::new(vec![0]).with_initial(0).allowing_unassigned();
        let p = FiniteProblem::new(vec![v.clone().with_pinned(true), v.clone(), v]).unwrap();
        let a = p.initial_assignment();
        let m: Vec<_> = mass_unassign_moves(&p, &a, MassAssignmentSize::exactly(2))
            .unwrap()
            .take(50)
            .collect();
        assert!(!m.is_empty());
        assert!(m.iter().all(|m| !m.members().contains(&0)));
    }
}

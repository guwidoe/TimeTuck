// Adapted from Timefold Community SolutionManagerTest.updateEverythingList.
// Copyright Timefold BV and contributors. Apache-2.0; see THIRD_PARTY_NOTICES.md
// and compat/provenance.json for source fixtures and modifications.
use timetuck::*;
#[test]
fn update_everything_list() {
    let problem = ListProblem::new(
        vec!["a0", "a1", "b0", "b1", "b2", "c0"],
        vec![
            ListEntity::new(vec!["a0", "a1"]),
            ListEntity::new(vec!["b0", "b1", "b2"]),
            ListEntity::new(vec!["c0"]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut network = ListConstraintNetwork::<&str, SimpleScore>::new(4, 6);
    for entity in 0..4 {
        network
            .add_constraint(
                format!("testConstraint/{entity}"),
                vec![ListScope::Entity(entity)],
                |facts| {
                    let [ListFact::Entity(row)] = facts else {
                        unreachable!()
                    };
                    let size = row.len() as i64;
                    SimpleScore::new(-size * size)
                },
            )
            .unwrap();
    }
    let mut solution = ManagedSolution::new(
        problem.initial_assignment(),
        vec![ListBuiltinValues::default(); 6],
    );
    assert!(solution.score.is_none());
    for shadow in &solution.shadows {
        assert_eq!(shadow.index, None);
        assert_eq!(shadow.owner, None);
        assert_eq!(shadow.previous, None);
        assert_eq!(shadow.next, None);
    }
    // Explicit native construction replaces both Java factory creation routes.
    let mut manager = list_solution_manager(&problem, &network);
    manager.update(&mut solution).unwrap();
    assert!(solution.score.is_some());
    assert_eq!(solution.score, Some(SimpleScore::new(-14)));
    for (position, (owner, index, previous, next)) in [
        (0, 0, None, Some("a1")),
        (0, 1, Some("a0"), None),
        (1, 0, None, Some("b1")),
        (1, 1, Some("b0"), Some("b2")),
        (1, 2, Some("b1"), None),
        (2, 0, None, None),
    ]
    .into_iter()
    .enumerate()
    {
        let actual = &solution.shadows[position];
        assert_eq!(actual.index, Some(index));
        assert_eq!(actual.owner, Some(owner));
        assert_eq!(actual.previous, previous);
        assert_eq!(actual.next, next);
    }
}

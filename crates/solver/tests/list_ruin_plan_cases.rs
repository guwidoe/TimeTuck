// Adapted from Timefold Community SelectorBasedListRuinRecreateMoveTest,
// executeAndUndoNewDestinationEntityWithPinnedPrefix (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for attribution.
use timetuck::*;

#[test]
fn execute_and_undo_new_destination_entity_with_pinned_prefix() {
    let problem = ListProblem::new_indexed(
        vec!["aPin", "special1", "special2", "special3", "bPin"],
        vec![
            ListEntity::new(vec!["aPin", "special1", "special2", "special3"]).with_pinned_prefix(1),
            ListEntity::new(vec!["bPin"]).with_pinned_prefix(1),
        ],
    )
    .unwrap();
    let before = problem.initial_assignment();
    let mut network = ListConstraintNetwork::<&str, SimpleScore>::new(2, 5);
    network
        .add_constraint("specials on B", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            SimpleScore::new(
                -(row
                    .iter()
                    .filter(|value| value.starts_with("special"))
                    .count() as i64),
            )
        })
        .unwrap();
    let plan = ListRuinPlan::new(&problem, &before, &[1, 2, 3]).unwrap();
    let rebuilt = greedy_list_construction(
        plan.repair_problem(),
        &network,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(rebuilt.complete);
    let movement = plan.finish(&rebuilt.assignment).unwrap();
    let mut director = ListScoreDirector::new(&problem, &network, before.clone()).unwrap();
    TemporaryMoves::new(&mut director)
        .process(
            &movement,
            TemporaryMoveOptions {
                restored_score: RestoredScorePolicy::CheckFresh,
                ..Default::default()
            },
            |assignment, score| {
                // Additional native checks ensure the test really exercises the
                // previously untouched receiver before the two source assertions.
                assert_eq!(score, SimpleScore::new(0));
                assert_eq!(assignment.list(0), Some(["aPin"].as_slice()));
                for special in ["special1", "special2", "special3"] {
                    assert_eq!(assignment.location(&special).unwrap().entity, 1);
                }
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    assert_eq!(
        director.assignment().list(0),
        Some(["aPin", "special1", "special2", "special3"].as_slice())
    );
    assert_eq!(director.assignment().list(1), Some(["bPin"].as_slice()));
    director.assert_consistent().unwrap();
}

// Adapted from pinned ListRuinRecreateMoveSelectorTest.testRuiningOnlyOneAssignedValues.
// Apache-2.0; see compat/provenance.json for method and fixture attribution.
use timetuck::*;
#[test]
fn ruining_only_one_assigned_value() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .allowing_unassigned();
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("Only One Assigned", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(list) = v[0] else {
            unreachable!()
        };
        SimpleScore::new(-i64::from(list.len() != 1))
    })
    .unwrap();
    let c = RuinRecreateConfig {
        minimum_ruined: 3,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    };
    let a = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
    let b = ruin_recreate_list_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            b.score("Only One Assigned", s.entities.port(), |(_, list)| {
                SimpleScore::new(-i64::from(list.len() != 1))
            })?;
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        c,
    )
    .unwrap();
    for r in [a, b] {
        assert_eq!(r.best.unwrap().assignment.list(0).unwrap().len(), 1);
    }
}

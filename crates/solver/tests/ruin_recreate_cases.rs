// Adapted from Timefold Community RuinRecreateMoveSelectorTest.testRuiningNoAssignedValues.
// Apache-2.0; method and optional-domain fixtures recorded in compat/provenance.json.
use timetuck::*;
#[test]
fn ruining_no_assigned_values() {
    // Three optional entities, three shared values, initially one distinct value each.
    let p = FiniteProblem::new(
        (0..3)
            .map(|i| {
                Variable::new(vec![0, 1, 2])
                    .allowing_unassigned()
                    .with_initial(i)
            })
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("No Assigned", vec![0, 1, 2], |v| {
        SimpleScore::new(-(v.iter().filter(|v| v.is_some()).count() as i64))
    })
    .unwrap();
    let c = RuinRecreateConfig {
        minimum_ruined: 3,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    };
    let scoped = ruin_recreate_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
    let stream = ruin_recreate_stream_search_from(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("No Assigned", source.port(), |(_, v)| {
                SimpleScore::new(-i64::from(v.is_some()))
            })?;
            Ok((b.build(), source))
        },
        p.initial_assignment(),
        c,
    )
    .unwrap();
    for result in [scoped, stream] {
        let solution = result.best.unwrap();
        assert_eq!(solution.assignment.get(0), Some(&None));
        assert_eq!(solution.assignment.get(1), Some(&None));
        assert_eq!(solution.assignment.get(2), Some(&None));
    }
}

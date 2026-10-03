// Adapted from Timefold Community 2.7.0 FollowerSetValuesShadowVariableTest,
// Apache-2.0. Fixture attribution: compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "../examples/collection_follower_support/mod.rs"]
mod support;
use timetuck::*;
#[test]
fn test_move() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    let (g, s, shadow) = support::graph(
        &[vec![0, 1], vec![0, 2], vec![1, 2]],
        &["1".into(), "2".into()],
    );
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for (leader, value, expected) in [
        (0, 0, [Some(0), Some(0), None]),
        (1, 1, [Some(0), Some(0), Some(1)]),
        (2, 0, [Some(0), Some(0), Some(0)]),
    ] {
        d.apply(&Move::Change {
            variable: leader,
            value: Some(value),
        })
        .unwrap()
        .commit();
        let m = d.graph().constraint_matches(&shadow).unwrap();
        assert_eq!([m[&0].0.1, m[&1].0.1, m[&2].0.1], expected);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn test_solve() {
    // Exact distinct Random(0) leader pairs from generateSolution(3,8,2),
    // including repeated draws when both candidate leader identities match.
    let dependencies = vec![
        vec![0, 1],
        vec![1, 2],
        vec![2, 0],
        vec![0, 2],
        vec![2, 1],
        vec![2, 1],
        vec![0, 2],
        vec![2, 1],
    ];
    let codes = vec!["Value 0".to_string(), "Value 1".to_string()];
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    let result = solve_stream(
        &p,
        || {
            let (g, s, _) = support::graph(&dependencies, &codes);
            Ok((g, s))
        },
        SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(1000),
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            }],
        },
    )
    .unwrap();
    assert!(result.complete);
    assert!(result.work.move_evaluations <= 1000);
    let (g, s, shadow) = support::graph(&dependencies, &codes);
    let d = StreamScoreDirector::new(&p, g, s, result.assignment.clone()).unwrap();
    assert_eq!(Some(d.score()), result.score);
    let m = d.graph().constraint_matches(&shadow).unwrap();
    for (id, refs) in dependencies.iter().enumerate() {
        let mut minimum = None;
        for &leader in refs {
            let value = result.assignment.values()[leader];
            if minimum.is_none() || value.is_some_and(|v| codes[v] < codes[minimum.unwrap()]) {
                minimum = value;
            }
        }
        assert_eq!(m[&id].0.1, minimum);
    }
}

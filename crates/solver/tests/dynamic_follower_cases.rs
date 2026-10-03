// Adapted from Timefold Community 2.7.0 DynamicFollowerValuesShadowVariableTest
// and testdomain/shadow/dynamic_follower fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
type Shadow = StreamConstraint<usize, ((usize, Option<usize>), Option<usize>)>;
fn graph(
    leaders: usize,
) -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<usize>,
    Shadow,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let targets = b.filter(s.port(), move |r| r.0 < leaders).unwrap();
    let followers = b.filter(s.port(), move |r| r.0 >= leaders).unwrap();
    let followed = b.follow_reference(&followers, &targets, |r| r.1).unwrap();
    let values = b
        .map(&followed, |(f, t)| (*f, t.and_then(|t| t.1)))
        .unwrap();
    let shadow = b.score("derived", &values, |_| SimpleScore::ZERO).unwrap();
    // The pinned interface-based forEach includes all implementing entities,
    // including null genuine variables; verified by the public Java adapter.
    let follower_values = b.map(&values, |(_, v)| *v).unwrap();
    let leader_values = b.map(&targets, |r| r.1).unwrap();
    let all = b.concat(&leader_values, &follower_values).unwrap();
    let counts = b.group_collect(&all, |v| (*v, 1)).unwrap();
    b.score("Minimize value count", &counts, |(_, n)| {
        SimpleScore::new(-(n.count as i64).pow(2))
    })
    .unwrap();
    (b.build(), s, shadow)
}
fn problem(leaders: usize, followers: usize, initial: Vec<Option<usize>>) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..leaders + followers)
            .map(|i| {
                let v = Variable::new(if i < leaders {
                    vec![0, 1]
                } else {
                    (0..leaders).collect()
                });
                initial[i].map_or(v.clone(), |value| v.with_initial(value))
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn test_leader_value_change_move() {
    let p = problem(2, 3, vec![None, None, Some(0), Some(0), Some(1)]);
    let (g, s, shadow) = graph(2);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for (leader, value, expected) in [
        (0, 0, [Some(0), Some(0), None]),
        (1, 1, [Some(0), Some(0), Some(1)]),
        (0, 1, [Some(1), Some(1), Some(1)]),
    ] {
        d.apply(&Move::Change {
            variable: leader,
            value: Some(value),
        })
        .unwrap()
        .commit();
        let m = d.graph().constraint_matches(&shadow).unwrap();
        assert_eq!([m[&2].0.1, m[&3].0.1, m[&4].0.1], expected);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn test_leader_change_move() {
    let p = problem(2, 3, vec![Some(0), Some(1), Some(0), Some(0), Some(1)]);
    let (g, s, shadow) = graph(2);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for (follower, value, expected) in [
        (2, Some(1), [Some(1), Some(0), Some(1)]),
        (4, Some(0), [Some(1), Some(0), Some(0)]),
        (2, None, [None, Some(0), Some(0)]),
    ] {
        d.apply(&Move::Change {
            variable: follower,
            value,
        })
        .unwrap()
        .commit();
        let m = d.graph().constraint_matches(&shadow).unwrap();
        assert_eq!([m[&2].0.1, m[&3].0.1, m[&4].0.1], expected);
        d.assert_consistent().unwrap();
    }
}
#[test]
fn test_solve() {
    let p = problem(3, 8, vec![None; 11]);
    let config = SolverConfig {
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
    };
    let result = solve_stream(
        &p,
        || {
            let (g, s, _) = graph(3);
            Ok((g, s))
        },
        config,
    )
    .unwrap();
    assert!(result.complete);
    assert!(result.work.move_evaluations <= 1000);
    let (g, s, shadow) = graph(3);
    let d = StreamScoreDirector::new(&p, g, s, result.assignment.clone()).unwrap();
    assert_eq!(Some(d.score()), result.score);
    let m = d.graph().constraint_matches(&shadow).unwrap();
    for i in 3..11 {
        let leader = result.assignment.values()[i].unwrap();
        assert_eq!(m[&i].0.1, result.assignment.values()[leader]);
    }
}

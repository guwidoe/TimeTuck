// Adapted from Timefold Community 2.7.0 FollowerValuesShadowVariableTest and
// testdomain/shadow/follower fixtures, Apache-2.0. See compat/provenance.json.
use std::cell::RefCell;
use timetuck::*;
type Shadow = StreamConstraint<usize, ((usize, usize), Option<usize>)>;
fn graph<'a>(
    leaders: &[usize],
    calls: &'a RefCell<Vec<usize>>,
) -> (
    StreamGraph<'a, SimpleScore>,
    PlanningStreamSource<usize>,
    Shadow,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let followers = b.source::<usize, (usize, usize)>();
    let values = b
        .follow_shared_reference(
            followers.port(),
            source.port(),
            |r| r.1,
            |f, leader| {
                calls.borrow_mut().push(f.0);
                leader.and_then(|r| r.1)
            },
        )
        .unwrap();
    let shadow = b.score("shadows", &values, |_| SimpleScore::ZERO).unwrap();
    let follower_values = b.map(&values, |(_, v)| *v).unwrap();
    let leader_values = b.map(source.port(), |r| r.1).unwrap();
    // Interface-based forEach includes null genuine and shadow values in the
    // pinned public runtime; see generated follower comparisons.
    let all = b.concat(&follower_values, &leader_values).unwrap();
    let counts = b.group_collect(&all, |v| (*v, 1)).unwrap();
    b.score("Minimize value count", &counts, |(_, n)| {
        SimpleScore::new(-(n.count as i64).pow(2))
    })
    .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, &leader) in leaders.iter().enumerate() {
        t.apply(&followers, TupleChange::put(id, (id, leader)))
            .unwrap();
    }
    t.commit().unwrap();
    (g, source, shadow)
}
#[test]
fn test_move() {
    let calls = RefCell::new(vec![]);
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let (g, s, shadow) = graph(&[0, 0, 0, 1, 1], &calls);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-64).with_structural(-3));
    for (leader, value, expected, score) in [
        (0, 1, [Some(1), Some(1), Some(1), None, None], -32),
        (1, 2, [Some(1), Some(1), Some(1), Some(2), Some(2)], -26),
        (2, 1, [Some(1), Some(1), Some(1), Some(2), Some(2)], -34),
    ] {
        d.apply(&Move::Change {
            variable: leader,
            value: Some(value),
        })
        .unwrap()
        .commit();
        let actual: Vec<_> = d
            .graph()
            .constraint_matches(&shadow)
            .unwrap()
            .values()
            .map(|((_, value), _)| *value)
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(
            d.score(),
            SimpleScore::new(score).with_structural(leader as i64 - 2)
        );
        d.assert_consistent().unwrap();
    }
}
#[test]
fn test_update_once() {
    let calls = RefCell::new(vec![]);
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let (g, s, _) = graph(&[0, 0, 0, 1, 1], &calls);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    calls.borrow_mut().clear();
    d.apply(&Move::Change {
        variable: 0,
        value: Some(1),
    })
    .unwrap()
    .commit();
    assert_eq!(*calls.borrow(), vec![0]); // A1 once; A2/A3/B1/B2 never.
}
#[test]
fn test_solve() {
    // Exact Random(0).nextInt(3) outputs for generateSolution(3, 8, 2).
    let followers = [0, 1, 1, 2, 2, 2, 2, 0];
    let calls = RefCell::new(vec![]);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    let result = solve_stream(
        &p,
        || {
            let (g, s, _) = graph(&followers, &calls);
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
    let (g, s, shadow) = graph(&followers, &calls);
    let d = StreamScoreDirector::new(&p, g, s, result.assignment.clone()).unwrap();
    assert_eq!(Some(d.score()), result.score);
    let matches = d.graph().constraint_matches(&shadow).unwrap();
    for (id, &leader) in followers.iter().enumerate() {
        assert_eq!(matches[&id].0.1, result.assignment.values()[leader]);
    }
}

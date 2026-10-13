use timetuck::*;
fn key(score: i64, bound: Option<i64>) -> SearchNodeKey<SimpleScore> {
    SearchNodeKey {
        depth: 0,
        score: SimpleScore::new(score),
        optimistic_bound: bound.map(SimpleScore::new),
        parent_breadth: 0,
        breadth: 0,
    }
}
#[test]
fn actual_branches_survive_unknown_bounds_and_execute_after_incumbent_pruning() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2])]).unwrap();
    let network = ConstraintNetwork::<i64, SimpleScore>::new(1);
    let before = problem.initial_assignment();
    let mut director = ScoreDirector::new(&problem, &network, before.clone()).unwrap();
    let mut frontier = PruningFrontier::new(NodeExploration::ScoreFirst);
    for value in 0..3 {
        let movement = Move::Change {
            variable: 0,
            value: Some(value),
        };
        let (_, record) = TemporaryMoves::new(&mut director)
            .recorded(&movement, TemporaryMoveOptions::default(), |_, _| {
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(frontier.push(key(value, Some(value)), record));
    }
    assert_eq!(frontier.register_bound(SimpleScore::new(1)).unwrap(), 2);
    assert_eq!(frontier.register_bound(SimpleScore::new(0)).unwrap(), 0);
    let (metadata, record) = frontier.pop().unwrap();
    let mut assignment = before;
    let movement = record.forward(&assignment).unwrap();
    problem.apply(&mut assignment, movement).unwrap().commit();
    assert_eq!(
        SimpleScore::new(assignment.values()[0].unwrap()),
        metadata.score
    );
    assert!(frontier.is_empty());
    assert!(!frontier.push(key(0, Some(1)), record));
    assert!(frontier.push(key(-100, None), record_for(&problem, &network)));
    assert_eq!(frontier.register_bound(SimpleScore::new(2)).unwrap(), 0);
    assert_eq!(frontier.len(), 1);
}
fn record_for(
    p: &FiniteProblem<i64>,
    n: &ConstraintNetwork<i64, SimpleScore>,
) -> RecordedMove<Assignment<i64>, Move<i64>> {
    let mut d = ScoreDirector::new(p, n, p.initial_assignment()).unwrap();
    TemporaryMoves::new(&mut d)
        .recorded(
            &Move::Change {
                variable: 0,
                value: Some(0),
            },
            TemporaryMoveOptions::default(),
            |_, _| Ok::<_, ()>(()),
        )
        .unwrap()
        .1
}
#[test]
fn scheduling_ties_are_stable_and_invalid_or_weaker_incumbents_preserve_queue() {
    for order in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
        NodeExploration::InputOrderDepthFirst,
    ] {
        let mut frontier = PruningFrontier::new(order);
        for value in 0..10 {
            assert!(frontier.push(key(0, None), value));
        }
        let invalid = SimpleScore::new(99).with_structural(-1);
        assert_eq!(
            frontier.register_bound(invalid),
            Err(UninitializedPessimisticBound)
        );
        assert!(frontier.incumbent().is_none());
        assert_eq!(frontier.register_bound(SimpleScore::ZERO).unwrap(), 0);
        assert_eq!(frontier.register_bound(SimpleScore::new(-1)).unwrap(), 0);
        assert_eq!(frontier.incumbent(), Some(&SimpleScore::ZERO));
        let actual: Vec<_> =
            std::iter::from_fn(|| frontier.pop().map(|(_, value)| value)).collect();
        assert_eq!(actual, (0..10).collect::<Vec<_>>());
    }
}

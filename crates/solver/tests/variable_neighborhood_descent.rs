//! Original descent priority, equal-score stopping and budget regressions.
use timetuck::{variable_neighborhood_descent, *};

#[test]
fn neighborhood_priority_beats_later_higher_scores_and_limits_still_apply() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("landscape", vec![0, 1], |v| {
        SimpleScore::new(match (v[0], v[1]) {
            (Some(0), Some(1)) => 0,
            (Some(2), Some(1)) => 1,  // Earlier change neighborhood.
            (Some(1), Some(0)) => 10, // Later swap neighborhood.
            _ => -10,
        })
    })
    .unwrap();
    for (children, expected) in [
        ([(Neighborhood::Change, 1), (Neighborhood::Swap, 1000)], 1),
        ([(Neighborhood::Swap, 1), (Neighborhood::Change, 1000)], 10),
        ([(Neighborhood::Change, 0), (Neighborhood::Swap, 1)], 10),
    ] {
        let neighborhoods = NeighborhoodUnion {
            children: &children,
            order: UnionOrder::Sequential,
        };
        let r = local_search(
            &p,
            &n,
            LocalSearchConfig {
                max_steps: 1,
                assert_scores: true,
                ..variable_neighborhood_descent(&neighborhoods).unwrap()
            },
        )
        .unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::new(expected));
        assert_eq!(r.completed_steps, 1);
        assert_eq!(r.termination, LocalSearchTermination::StepLimit);
        let r = local_search(
            &p,
            &n,
            LocalSearchConfig {
                max_move_evaluations: Some(0),
                ..variable_neighborhood_descent(&neighborhoods).unwrap()
            },
        )
        .unwrap();
        assert_eq!(r.best.unwrap().score, SimpleScore::ZERO);
        assert_eq!(r.completed_steps, 0);
        assert_eq!(r.termination, LocalSearchTermination::MoveEvaluationLimit);
    }
    assert_eq!(p.initial_assignment().values(), &[Some(0), Some(1)]);
}

#[test]
fn ties_empty_neighborhoods_and_disabled_children_stop_without_moves() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let choices: &[&[(Neighborhood, u32)]] = &[
        &[],
        &[(Neighborhood::Change, 0)],
        &[(Neighborhood::Change, 1), (Neighborhood::Swap, 1)],
    ];
    for children in choices {
        let neighborhoods = NeighborhoodUnion {
            children,
            order: UnionOrder::Sequential,
        };
        let r = local_search(
            &p,
            &n,
            variable_neighborhood_descent(&neighborhoods).unwrap(),
        )
        .unwrap();
        assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
        assert_eq!(r.completed_steps, 0);
        assert_eq!(r.termination, LocalSearchTermination::NoAcceptedMove);
    }
    let random = NeighborhoodUnion {
        children: &[(Neighborhood::Change, 1)],
        order: UnionOrder::Random { seed: 1 },
    };
    assert!(matches!(
        variable_neighborhood_descent::<SimpleScore>(&random),
        Err(DescentRequiresSequentialOrder)
    ));
}

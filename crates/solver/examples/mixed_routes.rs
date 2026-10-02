//! Jointly choose vehicle depots (basic variables) and ordered customer routes.
use timetuck::{
    Assignment, ConstructionConfig, FiniteProblem, HardSoftScore, ListAssignment, ListEntity,
    ListProblem, LocalSearchConfig, MixedAssignment, MixedConstraintNetwork,
    MixedConstructionConfig, MixedConstructionOrder, MixedFact, MixedLocalSearchConfig,
    MixedProblem, MixedScope, MoveSelection, Variable, exhaustive_mixed_search,
    greedy_mixed_construction, mixed_local_search_from,
};

fn score(state: &MixedAssignment<i64, usize>) -> HardSoftScore {
    let coordinates = [1_i64, 9, 10];
    let mut overload = 0;
    let mut distance = 0;
    for (vehicle, route) in state.lists().lists().iter().enumerate() {
        let depot = state.basic().values()[vehicle].expect("complete depot choice");
        overload += route.len().saturating_sub(2) as i64;
        let mut previous = depot;
        for &customer in route {
            distance += (coordinates[customer] - previous).abs();
            previous = coordinates[customer];
        }
        distance += (previous - depot).abs();
    }
    HardSoftScore::new(-overload, -distance)
}

fn main() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 10]), Variable::new(vec![0, 10])]).unwrap(),
        ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let result = exhaustive_mixed_search(&problem, score, 96);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 96);
    let best = result.best.unwrap();
    assert!(problem.is_complete(&best.assignment));
    assert_eq!(best.score, score(&best.assignment));
    // Customers 0 and 1 are each one unit from their nearest available depot.
    // In separate routes they each cost at least 2; in one route their span of
    // 8 costs at least 16. Therefore every solution has total distance >= 4.
    // Routes [customer 0] from depot 0 and [1, 2] from depot 10 attain this bound.
    assert_eq!(best.score, HardSoftScore::new(0, -4));
    println!(
        "{} after {} complete mixed assignments",
        best.score, result.evaluations
    );
    println!(
        "depots: {:?}; routes: {:?}",
        best.assignment.basic().values(),
        best.assignment.lists().lists()
    );
    let mut network = MixedConstraintNetwork::<i64, usize, HardSoftScore>::new(2, 2, 3);
    for vehicle in 0..2 {
        network
            .add_constraint(
                format!("route {vehicle}"),
                vec![MixedScope::Basic(vehicle), MixedScope::ListEntity(vehicle)],
                |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    let depot = depot.copied().unwrap_or(0);
                    let points = [1_i64, 9, 10];
                    let tour: Vec<_> = std::iter::once(depot)
                        .chain(route.iter().map(|&v| points[v]))
                        .chain(std::iter::once(depot))
                        .collect();
                    let distance: i64 = tour.windows(2).map(|edge| (edge[1] - edge[0]).abs()).sum();
                    HardSoftScore::new(-(route.len().saturating_sub(2) as i64), -distance)
                },
            )
            .unwrap();
    }
    let initial = MixedAssignment::new(
        Assignment::new(vec![Some(0), Some(0)]),
        ListAssignment::new(vec![vec![0, 1], vec![2]]),
    );
    let local = mixed_local_search_from(
        &problem,
        &network,
        initial,
        MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_evaluations: 1000,
                max_steps: 20,
                selection: MoveSelection::BestAccepted,
                assert_scores: true,
                ..Default::default()
            },
            joint_moves: true,
        },
    )
    .unwrap();
    let solution = local.best.unwrap();
    assert!(problem.is_complete(&solution.assignment));
    assert_eq!(solution.score, score(&solution.assignment));
    assert_eq!(solution.score, best.score);
    println!(
        "Local search: {} after {} evaluations / {} callbacks / {} steps",
        solution.score, local.evaluations, local.constraint_evaluations, local.completed_steps
    );
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let construction = greedy_mixed_construction(
            &problem,
            &network,
            MixedConstructionConfig {
                construction: ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                order,
            },
        )
        .unwrap();
        let constructed = construction.solution().expect("complete construction");
        assert_eq!(constructed.score, score(&constructed.assignment));
        let improved = mixed_local_search_from(
            &problem,
            &network,
            constructed.assignment,
            MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 1000,
                    max_steps: 20,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: true,
            },
        )
        .unwrap();
        let solution = improved.best.unwrap();
        assert!(problem.is_complete(&solution.assignment));
        assert_eq!(solution.score, score(&solution.assignment));
        assert_eq!(solution.score, best.score);
        println!(
            "{order:?}: {} constructed, {} improved; {} combined evaluations / {} callbacks",
            constructed.score,
            solution.score,
            construction.evaluations + improved.evaluations,
            construction.constraint_evaluations + improved.constraint_evaluations,
        );
    }
}

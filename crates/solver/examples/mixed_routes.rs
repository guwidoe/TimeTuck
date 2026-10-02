//! Jointly choose vehicle depots (basic variables) and ordered customer routes.
use timetuck::{
    FiniteProblem, HardSoftScore, ListEntity, ListProblem, MixedAssignment, MixedProblem, Variable,
    exhaustive_mixed_search,
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
}

//! Original ordered-list routing example with independently verifiable costs.
use timetuck::{HardSoftScore, ListAssignment, ListEntity, ListProblem, exhaustive_list_search};
fn score(a: &ListAssignment<i32>) -> HardSoftScore {
    let mut hard = 0;
    let mut distance = 0_i64;
    for (entity, route) in a.lists().iter().enumerate() {
        hard -= route.len().saturating_sub(3) as i64;
        let depot = if entity == 0 { 0 } else { 10 };
        let mut previous = depot;
        for &stop in route {
            distance += i64::from((previous - stop).abs());
            previous = stop;
        }
        distance += i64::from((previous - depot).abs());
    }
    HardSoftScore::new(hard, -distance)
}
fn main() {
    let p = ListProblem::new(
        vec![1, 2, 3, 7, 8, 9],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let result = exhaustive_list_search(&p, score, 10_000);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 5_040);
    let best = result.best.unwrap();
    assert!(p.is_complete(&best.assignment));
    assert_eq!(best.score, score(&best.assignment));
    assert_eq!(best.score, HardSoftScore::new(0, -12));
    println!(
        "{} · {} complete assignments",
        best.score, result.evaluations
    );
    for (entity, route) in best.assignment.lists().iter().enumerate() {
        println!("Vehicle {entity}: {route:?}");
    }
}

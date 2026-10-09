//! Original routing example: score indexed matrix paths against a direct oracle.
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut matrix = DirectedDistanceMatrix::with_capacity(2)?;
    let positions = [0i32, 10, 1, 2, 3, 7, 8, 9];
    let indexes: Vec<_> = positions
        .iter()
        .map(|&p| matrix.register(p))
        .collect::<Result<_, _>>()?;
    for (i, &a) in positions.iter().enumerate() {
        for (j, &b) in positions.iter().enumerate() {
            matrix.put_indexed(indexes[i], indexes[j], i64::from((a - b).abs()))?;
        }
    }
    let p = ListProblem::new(
        indexes[2..].to_vec(),
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )?;
    let score = |a: &ListAssignment<LocationIndex>| {
        let mut hard = 0;
        let mut distance = 0;
        for (e, route) in a.lists().iter().enumerate() {
            hard -= route.len().saturating_sub(3) as i64;
            distance += matrix
                .route_cost(indexes[e], route.iter().copied(), true)
                .unwrap();
        }
        HardSoftScore::new(hard, -distance)
    };
    let result = exhaustive_list_search(&p, score, 10000);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 5040);
    let best = result.best.unwrap();
    assert!(p.is_complete(&best.assignment));
    assert_eq!(best.score, HardSoftScore::new(0, -12));
    let mut reference = 0i64;
    for (e, route) in best.assignment.lists().iter().enumerate() {
        let depot = positions[e];
        let mut previous = depot;
        for &stop in route {
            let next = *matrix.location(stop)?;
            reference += i64::from((previous - next).abs());
            previous = next;
        }
        reference += i64::from((previous - depot).abs());
    }
    assert_eq!(best.score, HardSoftScore::new(0, -reference));
    println!(
        "{} · {} complete assignments · cached matrix indexes",
        best.score, result.evaluations
    );
    Ok(())
}

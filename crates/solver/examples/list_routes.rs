//! Original bounded list construction/search example; not a Java benchmark.
use timetuck::*;
const VEHICLES: usize = 4;
const CAPACITY: usize = 10;
fn main() {
    let values: Vec<i32> = (1..=CAPACITY as i32)
        .flat_map(|offset| (0..VEHICLES as i32).map(move |e| e * 100 + offset))
        .collect();
    let problem = ListProblem::new(
        values,
        (0..VEHICLES).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap();
    let mut network =
        ListConstraintNetwork::<i32, HardSoftScore>::new(VEHICLES, VEHICLES * CAPACITY);
    for e in 0..VEHICLES {
        network
            .add_constraint(format!("route {e}"), vec![ListScope::Entity(e)], move |f| {
                let ListFact::Entity(route) = &f[0] else {
                    unreachable!()
                };
                let depot = e as i32 * 100;
                let mut previous = depot;
                let mut distance = 0;
                for &v in *route {
                    distance += i64::from((v - previous).abs());
                    previous = v;
                }
                distance += i64::from((previous - depot).abs());
                HardSoftScore::new(-(route.len().saturating_sub(CAPACITY) as i64), -distance)
            })
            .unwrap();
    }
    let constructed = greedy_list_construction(
        &problem,
        &network,
        ConstructionConfig {
            max_evaluations: 2_000,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(constructed.complete);
    let improved = list_local_search_from(
        &problem,
        &network,
        constructed.assignment,
        LocalSearchConfig {
            max_evaluations: 5_000,
            max_steps: 20,
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        },
    )
    .unwrap();
    let best = improved.best.unwrap();
    assert!(problem.is_complete(&best.assignment));
    problem.validate(&best.assignment).unwrap();
    // Independent route formula and full identity check, outside the search budget.
    let mut identities: Vec<_> = best.assignment.lists().iter().flatten().copied().collect();
    identities.sort_unstable();
    let mut expected = problem.values().to_vec();
    expected.sort_unstable();
    assert_eq!(identities, expected);
    let mut distance = 0;
    for (e, route) in best.assignment.lists().iter().enumerate() {
        assert!(route.len() <= CAPACITY);
        let depot = e as i32 * 100;
        let points: Vec<_> = std::iter::once(depot)
            .chain(route.iter().copied())
            .chain(std::iter::once(depot))
            .collect();
        distance += points
            .windows(2)
            .map(|w| i64::from((w[0] - w[1]).abs()))
            .sum::<i64>();
    }
    assert_eq!(best.score, HardSoftScore::new(0, -distance));
    assert!(distance <= 80, "fixed-instance quality threshold");
    println!(
        "40 customers · 4 vehicles · {} · distance {}",
        best.score, distance
    );
    println!(
        "Construction: {} score evaluations, {} callbacks, {:?}",
        constructed.evaluations, constructed.constraint_evaluations, constructed.termination
    );
    println!(
        "Local search: {} score evaluations, {} callbacks, {} steps, {:?}",
        improved.evaluations,
        improved.constraint_evaluations,
        improved.completed_steps,
        improved.termination
    );
    println!("Bounded heuristic result; no global optimality proof or Java timing comparison.");
}

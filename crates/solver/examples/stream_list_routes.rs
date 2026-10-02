//! Original stream-scored list construction/search example; not a Java benchmark.
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
    let graph = || {
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let sources = ListStreamSources::new(&mut b);
        b.score("capacity", sources.entities.port(), |(_, route)| {
            HardSoftScore::new(-(route.len().saturating_sub(CAPACITY) as i64), 0)
        })
        .unwrap();
        let assigned = b
            .filter(sources.values.port(), |r| r.position.is_some())
            .unwrap();
        b.score("distance", &assigned, |r| {
            let depot = r.position.unwrap().entity as i32 * 100;
            let previous = r.previous.unwrap_or(depot);
            let distance = i64::from((r.value - previous).abs())
                + if r.next.is_none() {
                    i64::from((r.value - depot).abs())
                } else {
                    0
                };
            HardSoftScore::new(0, -distance)
        })
        .unwrap();
        (b.build(), sources)
    };
    let (g, s) = graph();
    let constructed = greedy_list_stream_construction(
        &problem,
        g,
        s,
        ConstructionConfig {
            max_evaluations: 2_000,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(constructed.complete);
    let (g, s) = graph();
    let improved = list_stream_local_search_from(
        &problem,
        g,
        s,
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

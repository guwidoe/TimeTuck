//! Original larger-than-exhaustive example using greedy construction and local search.
use timetuck::{
    ConstraintNetwork, ConstructionConfig, FiniteProblem, HardSoftScore, LocalSearchConfig,
    Neighborhood, Variable, greedy_construction, local_search_from,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count = 100;
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]); count])?;
    let mut constraints = ConstraintNetwork::new(count);
    // Ring neighbors up to distance three may not share a color.
    for i in 0..count {
        for distance in 1..=3 {
            constraints.add_constraint(
                format!("edge-{i}-{distance}"),
                vec![i, (i + distance) % count],
                |v| HardSoftScore::new(-i64::from(v[0].is_some() && v[0] == v[1]), 0),
            )?;
        }
        let preferred = i % 4;
        constraints.add_constraint(format!("preference-{i}"), vec![i], move |v| {
            HardSoftScore::new(0, -i64::from(v[0] != Some(&preferred)))
        })?;
    }
    let constructed = greedy_construction(&problem, &constraints, ConstructionConfig::default())?;
    println!(
        "Construction: {:?}; {} score evaluations; {} variables assigned",
        constructed.termination, constructed.evaluations, constructed.assigned_variables
    );
    let initial = constructed
        .solution()
        .ok_or("construction did not produce a scored complete assignment")?;
    let result = local_search_from(
        &problem,
        &constraints,
        initial.assignment,
        LocalSearchConfig {
            max_evaluations: 100_000,
            max_accepted_moves: 1_000,
            neighborhood: Neighborhood::Change,
            ..LocalSearchConfig::default()
        },
    )?;
    println!(
        "{:?}; {} score evaluations; {} accepted moves",
        result.termination, result.evaluations, result.accepted_moves
    );
    println!(
        "Combined: {} score evaluations; {} constraint callbacks (full rescoring would use {})",
        constructed.evaluations + result.evaluations,
        constructed.constraint_evaluations + result.constraint_evaluations,
        u128::from(constructed.evaluations + result.evaluations)
            * constraints.constraint_count() as u128
    );
    if let Some(best) = result.best {
        assert!(problem.is_complete(&best.assignment));
        assert_eq!(
            constraints.calculate(&problem, &best.assignment)?,
            best.score
        );
        println!(
            "Best: {} (feasible: {}); no global optimality claim",
            best.score,
            best.score.is_feasible()
        );
    }
    Ok(())
}

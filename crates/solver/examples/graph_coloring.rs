//! Original larger-than-exhaustive example using incremental local search.
use timetuck::{
    ConstraintNetwork, FiniteProblem, HardSoftScore, LocalSearchConfig, Neighborhood, Variable,
    local_search,
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
    let result = local_search(
        &problem,
        &constraints,
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
        "{} constraint callbacks (full rescoring would use {})",
        result.constraint_evaluations,
        u128::from(result.evaluations) * constraints.constraint_count() as u128
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

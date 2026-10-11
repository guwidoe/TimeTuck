//! Original incremental-stream heuristic benchmark with matched Java configuration.
mod solver_support;
#[allow(dead_code)]
mod stream_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::{
    Acceptance, LocalSearchConfig, LocalSearchTermination, MoveSelection, Neighborhood,
    RejectedMovePolicy, stream_local_search,
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(5..=7).contains(&args.len()) {
        return Err(
            "expected hill|tabu steps warmup samples [change|swap|swap-directed] [batched|immediate]".into(),
        );
    }
    let neighborhood = match args.get(5).map(String::as_str).unwrap_or("change") {
        "change" => Neighborhood::Change,
        "swap" => Neighborhood::UniqueSwap,
        "swap-directed" => Neighborhood::Swap,
        _ => return Err("unknown neighborhood".into()),
    };
    let batched = match args.get(6).map(String::as_str).unwrap_or("batched") {
        "batched" => true,
        "immediate" => false,
        _ => return Err("unknown graph mode".into()),
    };
    let acceptance = match args[1].as_str() {
        "hill" => Acceptance::HillClimbing,
        "tabu" => Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        _ => return Err("unknown algorithm".into()),
    };
    let steps: u64 = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let samples: usize = args[4].parse()?;
    if steps == 0 || samples == 0 {
        return Err("positive steps and samples required".into());
    }
    for line in io::stdin().lock().lines() {
        let case = solver_support::Case::parse(&line?, true)?;
        if !case.problem.is_complete(&case.problem.initial_assignment()) {
            return Err("complete initial assignment required".into());
        }
        let budget = evaluation_budget(&case, neighborhood.clone(), steps)?;
        let config = LocalSearchConfig {
            acceptance,
            max_accepted_moves: u64::MAX,
            max_steps: steps,
            rejected_moves: RejectedMovePolicy::BestScore,
            max_evaluations: budget,
            neighborhood: neighborhood.clone(),
            selection: MoveSelection::BestAccepted,
            ..LocalSearchConfig::default()
        };
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let started = Instant::now();
            let network = stream_support::build_with_batching(&case, batched)?;
            let result = stream_local_search(
                black_box(&case.problem),
                network.graph,
                network.source,
                config.clone(),
            )?;
            if matches!(
                result.termination,
                LocalSearchTermination::EvaluationLimit
                    | LocalSearchTermination::MoveEvaluationLimit
            ) {
                return Err("evaluation budget truncated the step-limited benchmark".into());
            }
            let best = result.best.ok_or("missing result")?;
            if stream_support::score(&case, &best.assignment) != best.score
                || !case.problem.is_complete(&best.assignment)
            {
                return Err("invalid result".into());
            }
            let elapsed = started.elapsed().as_nanos();
            if run >= warmup {
                let values = best
                    .assignment
                    .values()
                    .iter()
                    .map(|v| v.unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{}\t{elapsed}\t{}\t{}\t{values}\t{}",
                    case.id,
                    best.score.hard(),
                    best.score.soft(),
                    result.constraint_evaluations
                );
            }
        }
    }
    Ok(())
}

fn evaluation_budget(
    case: &solver_support::Case,
    neighborhood: Neighborhood,
    steps: u64,
) -> Result<u64, Box<dyn Error>> {
    let changes: u64 = case
        .problem
        .variables()
        .iter()
        .filter(|v| !v.is_pinned())
        .map(|v| (v.domain().len() - 1) as u64)
        .sum();
    let candidates = if matches!(neighborhood, Neighborhood::Swap | Neighborhood::UniqueSwap) {
        let movable = case
            .problem
            .variables()
            .iter()
            .filter(|v| !v.is_pinned())
            .count() as u64;
        movable
            .checked_mul(movable.saturating_sub(1))
            .ok_or("candidate count overflow")?
            / if neighborhood == Neighborhood::UniqueSwap {
                2
            } else {
                1
            }
    } else {
        changes
    };
    steps
        .checked_mul(candidates)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| "budget overflow".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap_budget_allows_every_requested_step() {
        let case = solver_support::Case::parse("budget\t2\t0,1\t00\t\t0,0,0,0", true).unwrap();
        for (neighborhood, evaluations) in [(Neighborhood::Swap, 9), (Neighborhood::UniqueSwap, 5)]
        {
            let network = stream_support::build_with_batching(&case, true).unwrap();
            let result = stream_local_search(
                &case.problem,
                network.graph,
                network.source,
                LocalSearchConfig {
                    acceptance: Acceptance::HillClimbing,
                    neighborhood: neighborhood.clone(),
                    max_steps: 4,
                    max_evaluations: evaluation_budget(&case, neighborhood, 4).unwrap(),
                    max_accepted_moves: u64::MAX,
                    selection: MoveSelection::BestAccepted,
                    ..LocalSearchConfig::default()
                },
            )
            .unwrap();
            assert_eq!(result.termination, LocalSearchTermination::StepLimit);
            assert_eq!(result.completed_steps, 4);
            assert_eq!(result.evaluations, evaluations);
        }
    }
}

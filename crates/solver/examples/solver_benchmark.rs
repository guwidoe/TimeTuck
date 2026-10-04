//! Original matched exact-search benchmark. Model parsing occurs before timing.
mod solver_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::{
    BranchAndBoundConfig, BranchEvaluation, NodeExploration, branch_and_bound, exhaustive_search,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 && args.len() != 5 {
        return Err("expected iterations warmup samples [brute|depth|breadth|score|bound]".into());
    }
    let iterations: usize = args[1].parse()?;
    let warmup: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    if iterations == 0 || samples == 0 {
        return Err("positive iterations and samples required".into());
    }
    let exploration = match args.get(4).map(String::as_str).unwrap_or("brute") {
        "brute" => None,
        "depth" => Some(NodeExploration::DepthFirst),
        "breadth" => Some(NodeExploration::BreadthFirst),
        "score" => Some(NodeExploration::ScoreFirst),
        "bound" => Some(NodeExploration::OptimisticBoundFirst),
        _ => return Err("unknown algorithm".into()),
    };
    let line = io::stdin().lock().lines().next().ok_or("missing case")??;
    let case = solver_support::Case::parse(&line, false)?;
    black_box(&case.id);
    if exploration.is_some()
        && (case.weights.iter().flatten().any(|&v| v > 0)
            || case
                .problem
                .variables()
                .iter()
                .any(|v| v.allows_unassigned()))
    {
        return Err("branch benchmark requires nonpositive weights and required variables".into());
    }
    let budget = case
        .problem
        .search_space_size()
        .ok_or("space overflow")?
        .try_into()?;
    for sample in 0..warmup + samples {
        let (mut hard, mut soft) = (0_i64, 0_i64);
        let mut last = None;
        let started = Instant::now();
        for _ in 0..iterations {
            let best = if let Some(exploration) = exploration {
                let result = branch_and_bound(
                    black_box(&case.problem),
                    |state, _| {
                        let score = case.score(state);
                        BranchEvaluation::bounded(score, score)
                    },
                    BranchAndBoundConfig {
                        exploration,
                        max_evaluations: u64::MAX,
                        max_frontier: 1_000_000,
                        ..Default::default()
                    },
                )?;
                if !result.is_optimal() {
                    return Err("incomplete branch result".into());
                }
                result.best.ok_or("missing branch result")?
            } else {
                let result =
                    exhaustive_search(black_box(&case.problem), |state| case.score(state), budget);
                if !result.is_optimal() {
                    return Err("incomplete result".into());
                }
                result.best.ok_or("missing result")?
            };
            if case.score(&best.assignment) != best.score
                || !case.problem.is_complete(&best.assignment)
            {
                return Err("invalid result".into());
            }
            hard += best.score.hard();
            soft += best.score.soft();
            last = Some(best);
        }
        let elapsed = started.elapsed().as_nanos();
        if sample >= warmup {
            let values = last
                .unwrap()
                .assignment
                .values()
                .iter()
                .map(|v| v.unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            println!("{elapsed}\t{hard}\t{soft}\t{values}");
        }
    }
    Ok(())
}

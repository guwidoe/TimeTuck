//! Original complete-planning oracle; not a user-facing solver CLI.
mod solver_support;
use std::{
    error::Error,
    io::{self, BufRead},
};
use timetuck::*;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let placement = match args.as_slice() {
        [mode] if mode == "pooled" => Some(ConstructionPlacement::Pooled),
        [mode] if mode == "value-queue" => Some(ConstructionPlacement::ValueQueue),
        _ => None,
    };
    let strategy = match args.as_slice() {
        [_] if placement.is_some() => None,
        [] => None,
        [order, trend] => Some(BoundedPhaseConfig {
            order: None,
            exploration: match order.as_str() {
                "depth" => NodeExploration::DepthFirst,
                "breadth" => NodeExploration::BreadthFirst,
                "score" => NodeExploration::ScoreFirst,
                "bound" => NodeExploration::OptimisticBoundFirst,
                _ => return Err("unknown branch order".into()),
            },
            max_frontier: 1_000_000,
            bound: PartialScoreBound::Trend(InitializingScoreTrend::parse(trend, 2)?),
        }),
        _ => return Err("expected pooled, value-queue, order and trend, or no arguments".into()),
    };
    for line in io::stdin().lock().lines() {
        let line = line?;
        if (placement.is_some() || strategy.is_some()) && line.split('\t').count() != 6 {
            return Err("trend adapter requires homogeneous required domains".into());
        }

        let case = solver_support::Case::parse(&line, false)?;
        let best = if let Some(placement) = placement {
            let len = case.problem.variables().len();
            let mut network = ConstraintNetwork::new(len);
            network.add_constraint("objective", (0..len).collect(), |v| {
                case.score_by(|i| v[i].copied())
            })?;
            let r = greedy_construction(
                &case.problem,
                &network,
                ConstructionConfig {
                    placement,
                    max_evaluations: u64::MAX,
                    ..Default::default()
                },
            )?;
            if r.termination != ConstructionTermination::Completed {
                return Err("incomplete construction".into());
            }
            r.solution().ok_or("missing constructed solution")?
        } else if let Some(strategy) = &strategy {
            let len = case.problem.variables().len();
            let mut network = ConstraintNetwork::new(len);
            network.add_constraint("objective", (0..len).collect(), |v| {
                case.score_by(|i| v[i].copied())
            })?;
            let r = solve(
                &case.problem,
                &network,
                SolverSequence {
                    phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                        strategy: ExactSearchStrategy::BranchAndBound(strategy.clone()),
                        max_evaluations: u64::MAX,
                        ..Default::default()
                    })],
                    ..Default::default()
                },
            )?;
            if !r.is_optimal() {
                return Err("expected optimal bounded result".into());
            }
            ScoredAssignment {
                assignment: r.assignment,
                score: r.score.ok_or("no score")?,
            }
        } else {
            let budget: u64 = case
                .problem
                .search_space_size()
                .ok_or("space overflow")?
                .try_into()?;
            let result = exhaustive_search(&case.problem, |state| case.score(state), budget);
            if !result.is_optimal() {
                return Err("expected a complete optimal result".into());
            }
            result.best.ok_or("no result")?
        };
        if case.score(&best.assignment) != best.score {
            return Err("score recomputation mismatch".into());
        }
        let values = best
            .assignment
            .values()
            .iter()
            .map(|v| v.map_or_else(|| "-".to_owned(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{}\t{}\t{}\t{}",
            case.id,
            best.score.hard(),
            best.score.soft(),
            values
        );
    }
    Ok(())
}

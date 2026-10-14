//! Original bounded repair quality/timing adapter; sampling differs from Java.
mod repair_scoring;
mod solver_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::{
    Acceptance, ConstructionPlacement, RuinRecreateConfig, RuinRecreateTermination, RuinSampling,
    ruin_recreate_search_from,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(5..=8).contains(&args.len()) {
        return Err(
            "group steps warmup samples [independent|coverage-cycle [sequential|cheapest [full|incremental|incremental-cached]]]".into(),
        );
    }
    let ruin_sampling = match args.get(5).map(String::as_str) {
        None | Some("independent") => RuinSampling::Independent,
        Some("coverage-cycle") => RuinSampling::CoverageCycle,
        _ => return Err("unknown repair sampling policy".into()),
    };
    let placement = match args.get(6).map(String::as_str) {
        None | Some("sequential") => ConstructionPlacement::Sequential,
        Some("cheapest") => ConstructionPlacement::Pooled,
        _ => return Err("unknown reconstruction placement".into()),
    };
    let scoring_model = repair_scoring::ScoringModel::parse(args.get(7).map(String::as_str))?;
    let callback_counts =
        std::env::var("TIMETUCK_REPAIR_CALLBACK_COUNTS").is_ok_and(|value| value == "1");
    let group: usize = args[1].parse()?;
    let steps: u64 = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let samples: usize = args[4].parse()?;
    if group == 0 || steps == 0 || steps >= i32::MAX as u64 || samples == 0 {
        return Err("invalid counts".into());
    }
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.split('\t').count() != 6 {
            return Err("homogeneous required variables only".into());
        }
        let case = solver_support::Case::parse(&line, true)?;
        let initial = case.problem.initial_assignment();
        if group > case.problem.variables().len()
            || initial.values().iter().any(|v| *v != Some(0))
            || case.problem.variables().iter().any(|v| v.is_pinned())
            || case.weights.iter().any(|w| w.iter().any(|v| *v < w[0]))
        {
            return Err("requires unpinned all-zero worst initial assignment".into());
        }
        let network = repair_scoring::network(&case, scoring_model)?;
        let placements = match placement {
            ConstructionPlacement::Sequential => group as u64,
            ConstructionPlacement::Pooled => {
                (group as u64)
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(group as u64))
                    .ok_or("placement count overflow")?
                    / 2
            }
            _ => unreachable!("only sequential and cheapest are accepted"),
        };
        let budget = steps
            .checked_mul(
                placements
                    .checked_mul(case.weights[0].len() as u64)
                    .and_then(|v| v.checked_add(1))
                    .ok_or("budget overflow")?,
            )
            .and_then(|v| v.checked_add(2))
            .ok_or("budget overflow")?;
        let config = RuinRecreateConfig {
            seed: 20261222,
            ruin_sampling,
            placement,
            minimum_ruined: group,
            maximum_ruined: group,
            max_attempts: steps,
            max_evaluations: budget,
            acceptance: Acceptance::StepCountingHillClimbing {
                period: steps + 1,
                mode: timetuck::StepCountingMode::Step,
            },
            ..Default::default()
        };
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let started = Instant::now();
            let result = ruin_recreate_search_from(
                black_box(&case.problem),
                &network,
                initial.clone(),
                config.clone(),
            )?;
            if result.attempts != steps
                || result.complete_reconstructions != steps
                || result.termination != RuinRecreateTermination::AttemptLimit
            {
                return Err("incomplete repair budget".into());
            }
            let best = result.best.ok_or("missing best")?;
            if !case.problem.is_complete(&best.assignment)
                || case.score(&best.assignment) != best.score
                || best.score < case.score(&initial)
            {
                return Err("invalid retained solution".into());
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
                    "{}\t{elapsed}\t{}\t{}\t{values}\t{}\t{}",
                    case.id,
                    best.score.hard(),
                    best.score.soft(),
                    result.evaluations,
                    result.attempts
                );
                if callback_counts {
                    eprintln!("{}\t{}", case.id, result.constraint_evaluations);
                }
            }
        }
    }
    Ok(())
}

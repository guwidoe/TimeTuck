//! Original bounded two-phase graph repair profile; no upstream method credit.
mod repair_scoring;
mod solver_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(5..=6).contains(&args.len()) {
        return Err("group attempts-per-phase warmup samples [full|incremental]".into());
    }
    let scoring = repair_scoring::ScoringModel::parse(args.get(5).map(String::as_str))?;
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
        let network = repair_scoring::network(&case, scoring)?;
        let domain = case.weights[0].len() as u64;
        let count = group as u64;
        let pooled = count
            .checked_add(1)
            .and_then(|n| n.checked_mul(count))
            .ok_or("placement count overflow")?
            / 2;
        let budget = |placements: u64| -> Result<u64, Box<dyn Error>> {
            steps
                .checked_mul(
                    placements
                        .checked_mul(domain)
                        .and_then(|n| n.checked_add(1))
                        .ok_or("phase budget overflow")?,
                )
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| "phase budget overflow".into())
        };
        let sequential_budget = budget(count)?;
        let pooled_budget = budget(pooled)?;
        let shared_budget = sequential_budget
            .checked_add(pooled_budget)
            .and_then(|n| n.checked_add(4))
            .ok_or("shared budget overflow")?;
        let first = RuinRecreateConfig {
            seed: 20261222,
            minimum_ruined: group,
            maximum_ruined: group,
            max_attempts: steps,
            max_evaluations: sequential_budget,
            acceptance: Acceptance::StepCountingHillClimbing {
                period: steps + 1,
                mode: StepCountingMode::Step,
            },
            ..Default::default()
        };
        let second = RuinRecreateConfig {
            ruin_sampling: RuinSampling::CoverageCycle,
            placement: ConstructionPlacement::Pooled,
            max_evaluations: pooled_budget,
            ..first.clone()
        };
        let plan = SolverSequence {
            limits: SolverLimits {
                max_evaluations: shared_budget,
                ..Default::default()
            },
            phases: vec![
                SolverPhase::RuinRecreate(first),
                SolverPhase::RuinRecreate(second),
            ],
        };
        let total_attempts = steps.checked_mul(2).ok_or("attempt count overflow")?;
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let started = Instant::now();
            let result = solve(black_box(&case.problem), &network, plan.clone())?;
            if result.work.repair_attempts != total_attempts
                || result.work.complete_reconstructions != total_attempts
                || result.termination != SolverTermination::PlanCompleted
                || result.phases.len() != 2
                || result.work.evaluations > shared_budget
                || result.phases.iter().any(|p| {
                    p.work.repair_attempts != steps
                        || p.termination
                            != PhaseTermination::RuinRecreate(RuinRecreateTermination::AttemptLimit)
                })
            {
                return Err("incomplete phased repair budget".into());
            }
            let score = result.score.ok_or("missing retained score")?;
            case.problem.validate(&result.assignment)?;
            if !result.complete
                || !case.problem.is_complete(&result.assignment)
                || case.score(&result.assignment) != score
                || score < case.score(&initial)
                || score < result.phases[0].score.ok_or("missing first-phase score")?
            {
                return Err("invalid retained phased solution".into());
            }
            let elapsed = started.elapsed().as_nanos();
            if run >= warmup {
                let values = result
                    .assignment
                    .values()
                    .iter()
                    .map(|v| v.unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{}\t{elapsed}\t{}\t{}\t{values}\t{}\t{}",
                    case.id,
                    score.hard(),
                    score.soft(),
                    result.work.evaluations,
                    result.work.repair_attempts
                );
            }
        }
    }
    Ok(())
}

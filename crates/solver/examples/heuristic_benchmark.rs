//! Original full-scoring heuristic benchmark with matched Java configuration.
mod solver_support;
use std::{
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::{
    Acceptance, ConstraintNetwork, FinalistRanking, LocalSearchConfig, MoveSelection, Neighborhood,
    RejectedMovePolicy, local_search,
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "expected hill|diversified|ratio_tabu|tabu|move_tabu|value_tabu|late_tabu|oscillation_last|oscillation_best|step|deluge steps warmup samples"
                .into(),
        );
    }
    let acceptance = match args[1].as_str() {
        "diversified" => Acceptance::DiversifiedLateAcceptance { history_size: 3 },
        "hill" => Acceptance::HillClimbing,
        "late_tabu" => Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: true,
        },
        "step" => Acceptance::StepCountingHillClimbing {
            period: 7,
            mode: timetuck::StepCountingMode::Step,
        },
        "deluge" => Acceptance::GreatDeluge {
            initial_water_level: None,
            rise: timetuck::GreatDelugeRise::Score(timetuck::HardSoftScore::new(0, 1)),
        },
        "value_tabu" => Acceptance::ValueTabu {
            tenure: 2,
            aspiration: true,
        },
        "move_tabu" => Acceptance::MoveTabu {
            tenure: 2,
            aspiration: true,
        },
        "ratio_tabu" => Acceptance::SizedVariableTabu {
            size: timetuck::TabuSize::ratio(0.2)?,
            aspiration: true,
        },
        "tabu" | "oscillation_last" | "oscillation_best" => Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        _ => return Err("unknown algorithm".into()),
    };
    let ranking = match args[1].as_str() {
        "oscillation_last" => FinalistRanking::StrategicOscillationLastStep,
        "oscillation_best" => FinalistRanking::StrategicOscillationBest,
        _ => FinalistRanking::HighestScore,
    };
    let extra_tabu = [Acceptance::VariableTabu {
        tenure: 2,
        aspiration: true,
    }];
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
        let mut network = ConstraintNetwork::new(case.problem.variables().len());
        network.add_constraint(
            "full-score",
            (0..case.problem.variables().len()).collect(),
            |v| case.score_by(|i| v[i].copied()),
        )?;
        let changes: u64 = case
            .problem
            .variables()
            .iter()
            .filter(|v| !v.is_pinned())
            .map(|v| (v.domain().len() - 1) as u64)
            .sum();
        let budget = steps
            .checked_mul(changes)
            .and_then(|v| v.checked_add(1))
            .ok_or("budget overflow")?;
        let config = LocalSearchConfig {
            acceptance,
            ranking,
            additional_acceptance: if args[1] == "late_tabu" {
                &extra_tabu
            } else {
                &[]
            },
            max_accepted_moves: u64::MAX,
            max_steps: steps,
            rejected_moves: RejectedMovePolicy::BestScore,
            max_evaluations: budget,
            neighborhood: Neighborhood::Change,
            selection: MoveSelection::BestAccepted,
            ..LocalSearchConfig::default()
        };
        for run in 0..warmup.checked_add(samples).ok_or("sample overflow")? {
            let started = Instant::now();
            let result = local_search(black_box(&case.problem), &network, config)?;
            let best = result.best.ok_or("missing result")?;
            if case.score(&best.assignment) != best.score
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
                    result.evaluations
                );
            }
        }
    }
    Ok(())
}

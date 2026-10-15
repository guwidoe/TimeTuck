//! Original mixed solve benchmark; parsing/independent validation outside timing.
mod mixed_benchmark_support;
use mixed_benchmark_support::{Case, run, run_cold};
use std::{
    error::Error,
    io::{self, BufRead},
    time::Instant,
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let cold = args.len() == 6 && args[5] == "cold";
    if args.len() != 5 && !cold {
        return Err("iterations warmup samples dense|stream [cold]".into());
    }
    let iterations: usize = args[1].parse()?;
    let warmup: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    let stream = match args[4].as_str() {
        "dense" => false,
        "stream" => true,
        _ => return Err("backend".into()),
    };
    if iterations == 0 || samples == 0 || iterations > 1000 || warmup > 100 || samples > 1000 {
        return Err("run bounds".into());
    }
    let line = io::stdin().lock().lines().next().ok_or("missing input")??;
    let case = Case::parse(&line)?;
    if cold != case.quality_floor.is_some() {
        return Err("cold mode requires a quality floor; warm mode uses its initial score".into());
    }
    for sample in 0..warmup + samples {
        let (mut elapsed, mut hard, mut soft, mut evaluations, mut calls, mut steps) =
            (0u128, 0i64, 0i64, 0u64, 0u128, 0u64);
        let mut last = None;
        let (mut construction_steps, mut phase_evaluations) = (0u64, [0u64; 2]);
        for _ in 0..iterations {
            let started = Instant::now();
            let (best, work, termination) = if cold {
                let result = run_cold(&case, stream)?;
                elapsed += started.elapsed().as_nanos();
                if !result.complete || result.phases.len() != 2 {
                    return Err("cold solve did not complete both phases".into());
                }
                construction_steps += result.work.construction_steps;
                for (sum, phase) in phase_evaluations.iter_mut().zip(&result.phases) {
                    *sum += phase.work.evaluations;
                }
                let best = timetuck::ScoredMixedAssignment {
                    assignment: result.assignment,
                    score: result.score.ok_or("missing score")?,
                };
                (best, result.work, format!("{:?}", result.termination))
            } else {
                let result = run(&case, stream)?;
                elapsed += started.elapsed().as_nanos();
                (
                    result.best.ok_or("missing best")?,
                    timetuck::SolverWork {
                        evaluations: result.evaluations,
                        constraint_evaluations: result.constraint_evaluations,
                        completed_steps: result.completed_steps,
                        ..Default::default()
                    },
                    format!("{:?}", result.termination),
                )
            };
            case.validate(&best.assignment, best.score)?;
            hard += best.score.hard();
            soft += best.score.soft();
            evaluations += work.evaluations;
            calls += work.constraint_evaluations;
            steps += work.completed_steps;
            last = Some((best, termination));
        }
        if sample >= warmup {
            let (best, termination) = last.unwrap();
            let mut row = serde_json::json!({"id":case.id,"elapsed_ns":u64::try_from(elapsed)?,"score":[hard,soft],"depots":best.assignment.basic().values(),"routes":best.assignment.lists().lists(),"evaluations":evaluations,"score_calls":u64::try_from(calls)?,"steps":steps,"termination":termination});
            if cold {
                row["construction_steps"] = construction_steps.into();
                row["phase_evaluations"] = phase_evaluations.into();
                row["complete"] = true.into();
            }
            println!("{row}");
        }
    }
    Ok(())
}

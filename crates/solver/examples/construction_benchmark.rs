//! Original matched construction benchmark. No upstream benchmark code is used.
mod solver_support;
use std::{
    cell::Cell,
    error::Error,
    hint::black_box,
    io::{self, BufRead},
    time::Instant,
};
use timetuck::*;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, warmup, samples] = args.as_slice() else {
        return Err("expected mode warmup samples".into());
    };
    let placement = match mode.as_str() {
        "sequential" => ConstructionPlacement::Sequential,
        "pooled" => ConstructionPlacement::Pooled,
        "value-queue" => ConstructionPlacement::ValueQueue,
        _ => return Err("unknown construction mode".into()),
    };
    let warmup: usize = warmup.parse()?;
    let samples: usize = samples.parse()?;
    let count = warmup.checked_add(samples).ok_or("sample count overflow")?;
    if samples == 0 {
        return Err("samples must be positive".into());
    }
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.split('\t').count() != 6 {
            return Err("benchmark requires homogeneous required domains".into());
        }
        let case = solver_support::Case::parse(&line, false)?;
        let calls = Cell::new(0_u64);
        let mut network = ConstraintNetwork::new(case.problem.variables().len());
        network.add_constraint(
            "objective",
            (0..case.problem.variables().len()).collect(),
            |v| {
                calls.set(calls.get() + 1);
                case.score_by(|i| v[i].copied())
            },
        )?;
        for sample in 0..count {
            calls.set(0);
            let started = Instant::now();
            let result = greedy_construction(
                black_box(&case.problem),
                &network,
                ConstructionConfig {
                    placement,
                    max_evaluations: u64::MAX,
                    ..Default::default()
                },
            )?;
            if result.termination != ConstructionTermination::Completed || !result.complete {
                return Err("incomplete construction".into());
            }
            case.problem.validate(&result.assignment)?;
            let score = result.score.ok_or("missing score")?;
            if score != case.score(&result.assignment) {
                return Err("incorrect score".into());
            }
            let elapsed = started.elapsed().as_nanos();
            if sample >= warmup {
                let values = result
                    .assignment
                    .values()
                    .iter()
                    .map(|v| v.unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    case.id,
                    elapsed,
                    score.hard(),
                    score.soft(),
                    values,
                    calls.get()
                );
            }
        }
    }
    Ok(())
}

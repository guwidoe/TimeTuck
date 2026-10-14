//! Original mixed solve benchmark; parsing/independent validation outside timing.
mod mixed_benchmark_support;
use mixed_benchmark_support::{Case, run};
use std::{
    error::Error,
    io::{self, BufRead},
    time::Instant,
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("iterations warmup samples dense|stream".into());
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
    for sample in 0..warmup + samples {
        let (mut elapsed, mut hard, mut soft, mut evaluations, mut calls, mut steps) =
            (0u128, 0i64, 0i64, 0u64, 0u128, 0u64);
        let mut last = None;
        for _ in 0..iterations {
            let started = Instant::now();
            let result = run(&case, stream)?;
            elapsed += started.elapsed().as_nanos();
            let best = result.best.ok_or("missing best")?;
            case.validate(&best.assignment, best.score)?;
            hard += best.score.hard();
            soft += best.score.soft();
            evaluations += result.evaluations;
            calls += result.constraint_evaluations;
            steps += result.completed_steps;
            last = Some((best, format!("{:?}", result.termination)));
        }
        if sample >= warmup {
            let (best, termination) = last.unwrap();
            println!(
                "{}",
                serde_json::json!({"id":case.id,"elapsed_ns":u64::try_from(elapsed)?,"score":[hard,soft],"depots":best.assignment.basic().values(),"routes":best.assignment.lists().lists(),"evaluations":evaluations,"score_calls":u64::try_from(calls)?,"steps":steps,"termination":termination})
            );
        }
    }
    Ok(())
}

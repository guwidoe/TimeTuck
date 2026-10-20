//! Application workload benchmark, separate from adapted upstream tests.
#[path = "support/real_world_model.rs"]
mod real_world_model;

use serde_json::{Value, json};
use std::{error::Error, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "usage: real_world_benchmark <problem.json> <seconds> <seed> <output.json>".into(),
        );
    }
    let problem: Value = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let seconds: f64 = args[2].parse()?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("seconds must be finite and positive".into());
    }
    let seed: u64 = args[3].parse()?;
    let (m, fp, initial) = real_world_model::load(&problem);
    let run = real_world_model::solve(
        &m,
        &fp,
        &initial,
        seed,
        Some(Duration::from_secs_f64(seconds)),
        u64::MAX,
    );
    let best = run
        .result
        .best
        .as_ref()
        .ok_or("solver returned no assignment")?;
    let (people, groups, sessions, variables) = m.dimensions();
    let summary = json!({
        "elapsed_ns": run.elapsed_ns,
        "evaluations": run.result.evaluations,
        "completed_steps": run.result.completed_steps,
        "accepted_moves": run.result.accepted_moves,
        "initial_score": [run.initial_score.hard(), run.initial_score.soft()],
        "best_score": [best.score.hard(), best.score.soft()],
        "termination": format!("{:?}", run.result.termination),
        "seed": seed, "seconds": seconds, "score_scale": 100,
        "starting_temperature": [0, 500], "cooling_steps": 40_000,
        "people": people, "groups": groups, "sessions": sessions, "variables": variables,
    });
    let mut output = problem;
    let object = output.as_object_mut().ok_or("problem must be an object")?;
    object.remove("schema_version");
    object.insert("incumbent_schedule".into(), m.schedule(&best.assignment));
    object.insert(
        "benchmark_initial_schedule".into(),
        m.schedule(&timetuck::Assignment::new(initial)),
    );
    object.insert(
        "solver_settings".into(),
        serde_json::from_str(include_str!(
            "../../../tests/application/real_world/solver-settings.json"
        ))?,
    );
    object.insert("benchmark_summary".into(), summary.clone());
    std::fs::write(&args[4], serde_json::to_string_pretty(&output)?)?;
    println!("{summary}");
    Ok(())
}

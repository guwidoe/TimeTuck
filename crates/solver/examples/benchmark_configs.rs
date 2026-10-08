//! Compare deterministic exact-search budgets on two small native problems.
//! Run with: cargo run --release --example benchmark_configs
//! This demonstrates the harness; it is not a comparison against Timefold.
use std::convert::Infallible;
use timetuck::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let problems = [6, 8]
        .into_iter()
        .map(|n| FiniteProblem::new((0..n).map(|_| Variable::new(vec![0_i64, 1, 2])).collect()))
        .collect::<Result<Vec<_>, _>>()?;
    let budgets = [10, 100, u64::MAX];
    let report =
        BenchmarkSchedule::default().run(&budgets, &problems, None, |&budget, problem, _| {
            let result = exhaustive_search(
                &problem,
                |a| {
                    SimpleScore::new(
                        -a.values()
                            .iter()
                            .map(|v| (2 - v.unwrap()).pow(2))
                            .sum::<i64>(),
                    )
                },
                budget,
            );
            Ok::<_, Infallible>(BenchmarkObservation {
                score: result
                    .best
                    .expect("nonempty domains and positive budget")
                    .score,
                initialized: true,
                work: SolverWork {
                    evaluations: result.evaluations,
                    move_evaluations: result.evaluations,
                    ..Default::default()
                },
            })
        })?;
    println!("budget,problem,samples,mean_ms,best_score");
    for (config, budget) in budgets.iter().enumerate() {
        for problem in 0..problems.len() {
            let timing = report.timing(config, problem).unwrap();
            let best = report
                .trials()
                .iter()
                .filter(|t| {
                    !t.trial.warmup && t.trial.configuration == config && t.trial.problem == problem
                })
                .filter_map(|t| t.result.as_ref().ok())
                .map(|r| r.score)
                .max()
                .unwrap();
            println!(
                "{budget},{problem},{},{:.6},{best}",
                timing.successful_runs,
                timing.mean_seconds().unwrap() * 1000.0
            );
        }
    }
    Ok(())
}

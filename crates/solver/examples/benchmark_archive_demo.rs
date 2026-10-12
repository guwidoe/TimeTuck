//! Original runnable example: solve three native models, archive and merge their journals.
use std::path::PathBuf;
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/benchmark-archive-demo"));
    let solver = SolverXmlConfiguration::parse(
        "<solver><solutionClass>demo.Plan</solutionClass><entityClass>demo.Task</entityClass><scoreDirectorFactory><easyScoreCalculatorClass>demo.Score</easyScoreCalculatorClass></scoreDirectorFactory></solver>",
    )?;
    let mut runtime = XmlBenchmarkRuntime::<FiniteProblem<i32>, SimpleScore, String>::default();
    runtime.register_solver(
        XmlFactorySignature::from_configuration(&solver)?,
        |_, problem, trial| {
            let mut network = ConstraintNetwork::new(problem.variables().len());
            network
                .add_constraint(
                    "assigned tasks",
                    (0..problem.variables().len()).collect(),
                    |values| {
                        SimpleScore::new(-(values.iter().filter(|v| v.is_some()).count() as i64))
                    },
                )
                .map_err(|e| e.to_string())?;
            let mut settings = SolverConfig::default();
            settings.limits.max_evaluations = 1000;
            let result = solve(&problem, &network, settings).map_err(|e| e.to_string())?;
            assert!(!trial.warmup);
            Ok(BenchmarkObservation {
                score: result.score.unwrap(),
                initialized: result.assignment.values().iter().all(Option::is_some),
                work: result.work,
            })
        },
    )?;
    let archive = BenchmarkArchive::at_directory(root.join("archive"));
    for i in 0..3 {
        let mut config =
            BenchmarkXmlConfiguration::from_solver(solver.clone(), Some(root.join("raw")));
        config.warmup_seconds = Some(0);
        let problem = FiniteProblem::new(
            (0..2 * (i + 1))
                .map(|_| Variable::new(vec![1, 2, 3]))
                .collect(),
        )?;
        let raw = runtime
            .build(&config, &[problem])?
            .benchmark()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        archive.archive_completed_report(&raw, &format!("2026-10-12_15000{i}"))?;
    }
    println!("{}", archive.aggregate_archived_runs()?.display());
    Ok(())
}

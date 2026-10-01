//! Original small scheduling example: two simultaneous jobs in each of three slots.
use timetuck::{Assignment, FiniteProblem, HardSoftScore, Variable, exhaustive_search};

fn score(state: &Assignment<usize>) -> HardSoftScore {
    let preferred = [0, 0, 1, 1, 2, 2];
    let mut hard = 0;
    let mut soft = 0;
    let mut loads = [0_i64; 3];
    for (job, worker) in state.values().iter().enumerate() {
        let worker = worker.expect("exact search supplies complete assignments");
        loads[worker] += 1;
        soft -= i64::from(worker != preferred[job]);
        // Adjacent pairs of jobs occur simultaneously.
        if job % 2 == 1 && state.values()[job - 1] == Some(worker) {
            hard -= 1;
        }
    }
    // Favor equal workload in addition to workers' preferred slots.
    soft -= loads.iter().map(|load| (load - 2).abs()).sum::<i64>();
    HardSoftScore::new(hard, soft)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut variables = vec![Variable::new(vec![0, 1, 2]); 6];
    variables[0] = Variable::new(vec![0, 1, 2])
        .with_initial(0)
        .with_pinned(true);
    let problem = FiniteProblem::new(variables)?;
    let result = exhaustive_search(&problem, score, 243);
    println!(
        "{} evaluations; {:?}; optimal: {}",
        result.evaluations,
        result.termination,
        result.is_optimal()
    );
    if let Some(best) = result.best {
        println!(
            "Score: {} (feasible: {})",
            best.score,
            best.score.is_feasible()
        );
        assert_eq!(score(&best.assignment), best.score);
        for (job, worker) in best.assignment.values().iter().enumerate() {
            println!(
                "Slot {}, job {}: worker {}",
                job / 2 + 1,
                job + 1,
                worker.unwrap() + 1
            );
        }
    }
    Ok(())
}

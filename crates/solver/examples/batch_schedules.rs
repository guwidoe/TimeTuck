//! Original asynchronous batch: real exact solves, owned results and explicit shutdown.
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SolverManager::<SearchResult<usize, HardSoftScore>, String>::new(2)?;
    let jobs: Vec<_> = (0..8)
        .map(|id| {
            manager.submit(id, move |token| {
                let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 6])
                    .map_err(|e| e.to_string())?;
                let preferred = id as usize % 3;
                let result = exhaustive_search_with_config(
                    &problem,
                    |a| {
                        let mut hard = 0;
                        let mut soft = 0;
                        for (i, v) in a.values().iter().enumerate() {
                            soft -= i64::from(*v != Some(preferred));
                            if i % 2 == 1 && *v == a.values()[i - 1] {
                                hard -= 1;
                            }
                        }
                        HardSoftScore::new(hard, soft)
                    },
                    ExactSearchConfig {
                        cancellation: Some(&token),
                        max_evaluations: 729,
                        ..Default::default()
                    },
                );
                Ok(result)
            })
        })
        .collect::<Result<_, _>>()?;
    for job in jobs {
        let result = job.wait();
        let solved = result.as_ref().as_ref().map_err(|e| e.to_string())?;
        assert!(solved.is_optimal());
        let best = solved.best.as_ref().unwrap();
        assert_eq!(best.score, HardSoftScore::new(0, -3));
        println!(
            "problem {}: {} ({:?}, {} evaluations)",
            job.problem_id(),
            best.score,
            job.status(),
            solved.evaluations
        );
    }
    manager.shutdown()?;
    Ok(())
}

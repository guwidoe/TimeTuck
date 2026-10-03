//! Original asynchronous batch: real exact solves, owned results and explicit shutdown.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SolverManager::<SearchResult<usize, HardSoftScore>, String>::new(2)?;
    let delivered = Arc::new(AtomicUsize::new(0));
    let jobs: Vec<_> = (0..8)
        .map(|id| {
            let delivered = delivered.clone();
            manager.submit_with_callbacks(
                id,
                move |token| {
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
                },
                SolverJobCallbacks::default().on_final(
                    move |_, result: &SearchResult<usize, HardSoftScore>| {
                        assert!(result.is_optimal());
                        delivered.fetch_add(1, Ordering::SeqCst);
                    },
                ),
            )
        })
        .collect::<Result<_, _>>()?;
    for job in jobs {
        let result = job.wait();
        assert!(job.callback_failures().is_empty());
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
    assert_eq!(delivered.load(Ordering::SeqCst), 8);
    println!("Delivered 8 final results on the consumer pool");
    manager.shutdown()?;
    Ok(())
}

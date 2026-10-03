//! Original live best-solution delivery from actual exact optimization.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SolverManager::<ScoredAssignment<usize, HardSoftScore>, String>::new(2)?;
    let updates = Arc::new(AtomicUsize::new(0));
    let mut jobs = Vec::new();
    for id in 0..8 {
        let seen = updates.clone();
        let mut previous = None;
        jobs.push(
            manager.submit_with_progress(
                id,
                move |token, progress| {
                    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 6])
                        .map_err(|e| e.to_string())?;
                    let r = exhaustive_search_with_observer(
                        &problem,
                        |a| {
                            let mut hard = 0;
                            let mut soft = 0;
                            for (i, v) in a.values().iter().enumerate() {
                                soft -= i64::from(*v != Some(id as usize % 3));
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
                        |best| {
                            assert!(progress.publish(best.clone()));
                        },
                    );
                    assert!(r.is_optimal());
                    r.best.ok_or_else(|| "no complete assignment".into())
                },
                SolverJobCallbacks::default()
                    .on_best(move |id, best: &ScoredAssignment<usize, HardSoftScore>| {
                        assert!(previous.is_none_or(|score| best.score > score));
                        previous = Some(best.score);
                        seen.fetch_add(1, Ordering::SeqCst);
                        println!("problem {id}: best {}", best.score);
                    })
                    .on_final(|id, best| {
                        assert_eq!(best.score, HardSoftScore::new(0, -3));
                        println!("problem {id}: final {}", best.score);
                    }),
            )?,
        );
    }
    for job in jobs {
        assert_eq!(
            job.wait()
                .as_ref()
                .as_ref()
                .map_err(|e| e.to_string())?
                .score,
            HardSoftScore::new(0, -3)
        );
        assert!(job.callback_failures().is_empty());
    }
    assert!(updates.load(Ordering::SeqCst) >= 8);
    manager.shutdown()?;
    println!("All 8 jobs delivered live updates and optimal final results");
    Ok(())
}

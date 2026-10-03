//! Original construction + improvement pipeline with live committed snapshots.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[derive(Clone)]
struct Snapshot {
    assignment: Assignment<usize>,
    score: SimpleScore,
    complete: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SolverManager::<Snapshot, String>::new(2)?;
    let mut jobs = Vec::new();
    for id in 0..4 {
        let consumed = Arc::new(AtomicUsize::new(0));
        let final_count = consumed.clone();
        let mut previous = None;
        jobs.push(
            manager.submit_with_progress(
                id,
                move |token, publisher| {
                    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 6])
                        .map_err(|e| e.to_string())?;
                    let mut n = ConstraintNetwork::new(6);
                    n.add_constraint("allocation value", (0..6).collect(), |v| {
                        SimpleScore::new(v.iter().flatten().map(|&&v| v as i64).sum())
                    })
                    .map_err(|e| e.to_string())?;
                    let mut retained = None;
                    let mut publish = |u: SolutionUpdate<'_, Assignment<usize>, SimpleScore>| {
                        if retained.is_none_or(|s| u.score > s) {
                            retained = Some(u.score);
                            assert!(publisher.publish(Snapshot {
                                assignment: u.assignment.clone(),
                                score: u.score,
                                complete: u.complete
                            }));
                        }
                    };
                    let c = greedy_construction_with_observer(
                        &problem,
                        &n,
                        ConstructionConfig {
                            cancellation: Some(&token),
                            pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
                            ..Default::default()
                        },
                        &mut publish,
                    )
                    .map_err(|e| e.to_string())?;
                    assert!(c.complete);
                    assert_eq!(c.score, Some(SimpleScore::ZERO));
                    let r = local_search_from_with_observer(
                        &problem,
                        &n,
                        c.assignment,
                        LocalSearchConfig {
                            cancellation: Some(&token),
                            max_steps: 20,
                            selection: MoveSelection::FirstAccepted,
                            ..Default::default()
                        },
                        &mut publish,
                    )
                    .map_err(|e| e.to_string())?;
                    let best = r.best.ok_or_else(|| "no scored solution".to_string())?;
                    Ok(Snapshot {
                        assignment: best.assignment,
                        score: best.score,
                        complete: true,
                    })
                },
                SolverJobCallbacks::default()
                    .on_best(move |id, s: &Snapshot| {
                        assert!(previous.is_none_or(|p| s.score > p));
                        previous = Some(s.score);
                        consumed.fetch_add(1, Ordering::SeqCst);
                        println!("job {id}: best {}, complete {}", s.score, s.complete);
                    })
                    .on_final(move |id, s| {
                        assert!(final_count.load(Ordering::SeqCst) > 0);
                        assert!(s.complete);
                        assert_eq!(s.score, SimpleScore::new(12));
                        assert_eq!(s.assignment.values(), &[Some(2); 6]);
                        println!("job {id}: final {}", s.score);
                    }),
            )?,
        );
    }
    for job in jobs {
        assert!(job.wait().is_ok());
        assert!(job.callback_failures().is_empty());
    }
    manager.shutdown()?;
    println!("All 4 construction/local-search jobs delivered improving snapshots and score 12");
    Ok(())
}

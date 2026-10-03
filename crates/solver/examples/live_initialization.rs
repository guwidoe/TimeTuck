//! Original managed construction-boundary and live-best example.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[derive(Clone)]
struct Snapshot {
    values: Vec<Option<i64>>,
    score: Option<SimpleScore>,
    complete: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let initialized = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let mut manager = SolverManager::<Snapshot, String>::with_consumer_count(2, 2)?;
    let mut jobs = Vec::new();
    for id in 0..4 {
        let initialized = initialized.clone();
        let finished = finished.clone();
        let mut last = None;
        jobs.push(
            manager.submit_with_progress(
                id,
                |token, progress| {
                    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 4]).unwrap();
                    let mut n = ConstraintNetwork::new(4);
                    n.add_constraint("sum", vec![0, 1, 2, 3], |v| {
                        SimpleScore::new(v.iter().filter_map(|v| v.copied()).sum())
                    })
                    .unwrap();
                    let c = SolverPlan {
                        limits: SolverLimits {
                            cancellation: Some(&token),
                            ..Default::default()
                        },
                        construction: ConstructionConfig {
                            max_evaluations: 2,
                            ..Default::default()
                        },
                        phases: vec![
                            SolverPhase::Construction(ConstructionConfig::default()),
                            SolverPhase::LocalSearch(LocalSearchConfig {
                                max_steps: 1,
                                selection: MoveSelection::FirstAccepted,
                                ..Default::default()
                            }),
                            SolverPhase::Exact(ExactPhaseConfig::default()),
                        ],
                    };
                    let r = solve_with_events(&p, &n, c, |event| match event {
                        SolverEvent::Best(u) => {
                            progress.publish(Snapshot {
                                values: u.assignment.values().to_vec(),
                                score: Some(u.score),
                                complete: u.complete,
                            });
                        }
                        SolverEvent::FirstInitialized {
                            initialization,
                            assignment,
                            score,
                            complete,
                        } => {
                            assert!(progress.publish_first_initialized(
                                Snapshot {
                                    values: assignment.values().to_vec(),
                                    score,
                                    complete
                                },
                                initialization
                            ));
                        }
                    })
                    .map_err(|e| e.to_string())?;
                    assert!(r.is_optimal());
                    assert_eq!(r.score, Some(SimpleScore::new(8)));
                    Ok(Snapshot {
                        values: r.assignment.values().to_vec(),
                        score: r.score,
                        complete: r.complete,
                    })
                },
                SolverJobCallbacks::default()
                    .on_first_initialized(move |id, s: &Snapshot, info| {
                        assert_eq!(
                            info,
                            SolverInitialization {
                                phase: 1,
                                kind: SolverPhaseKind::Construction,
                                terminated_early: false
                            }
                        );
                        assert!(s.complete);
                        assert_eq!(s.values, vec![Some(0), Some(2), Some(2), Some(2)]);
                        assert_eq!(s.score, Some(SimpleScore::new(6)));
                        initialized.fetch_add(1, Ordering::SeqCst);
                        println!("job {id}: initialized by construction phase {}", info.phase);
                    })
                    .on_best(move |_, s: &Snapshot| {
                        let score = s.score.unwrap();
                        assert!(last.is_none_or(|last| score > last));
                        last = Some(score);
                    })
                    .on_final(move |id, s: &Snapshot| {
                        assert!(s.complete);
                        assert_eq!(s.values, vec![Some(2); 4]);
                        assert_eq!(s.score, Some(SimpleScore::new(8)));
                        finished.fetch_add(1, Ordering::SeqCst);
                        println!("job {id}: proven optimum 8");
                    }),
            )?,
        );
    }
    for job in jobs {
        assert!(job.wait().is_ok());
        assert!(job.callback_failures().is_empty());
    }
    manager.shutdown()?;
    assert_eq!(initialized.load(Ordering::SeqCst), 4);
    assert_eq!(finished.load(Ordering::SeqCst), 4);
    println!("All four initialization events delivered before completion");
    Ok(())
}

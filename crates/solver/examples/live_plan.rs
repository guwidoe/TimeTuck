//! Original ordered construction/local/exact solve with one managed observer.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[derive(Clone)]
struct Snapshot {
    assignment: Assignment<usize>,
    score: SimpleScore,
    phase: usize,
    kind: SolverPhaseKind,
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
                    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 4])
                        .map_err(|e| e.to_string())?;
                    let mut n = ConstraintNetwork::new(4);
                    n.add_constraint("value", (0..4).collect(), |v| {
                        SimpleScore::new(v.iter().flatten().map(|&&v| v as i64).sum())
                    })
                    .map_err(|e| e.to_string())?;
                    let c = SolverPlan {
                        limits: SolverLimits {
                            cancellation: Some(&token),
                            ..Default::default()
                        },
                        construction: ConstructionConfig {
                            pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
                            ..Default::default()
                        },
                        phases: vec![
                            SolverPhase::LocalSearch(LocalSearchConfig {
                                max_steps: 1,
                                selection: MoveSelection::FirstAccepted,
                                ..Default::default()
                            }),
                            SolverPhase::Exact(ExactPhaseConfig::default()),
                        ],
                    };
                    let mut last = None;
                    let r = solve_with_observer(&p, &n, c, |u| {
                        let snapshot = Snapshot {
                            assignment: u.assignment.clone(),
                            score: u.score,
                            phase: u.phase,
                            kind: u.kind,
                        };
                        last = Some(snapshot.clone());
                        assert!(publisher.publish(snapshot));
                    })
                    .map_err(|e| e.to_string())?;
                    assert!(r.is_optimal());
                    let last = last.unwrap();
                    assert_eq!(r.score, Some(last.score));
                    assert_eq!(r.assignment, last.assignment);
                    Ok(last)
                },
                SolverJobCallbacks::default()
                    .on_best(move |id, s: &Snapshot| {
                        assert!(previous.is_none_or(|p| s.score > p));
                        previous = Some(s.score);
                        consumed.fetch_add(1, Ordering::SeqCst);
                        println!("job {id}: {:?} phase {} best {}", s.kind, s.phase, s.score);
                    })
                    .on_final(move |id, s| {
                        assert!(final_count.load(Ordering::SeqCst) > 0);
                        assert_eq!(s.score, SimpleScore::new(8));
                        assert_eq!(s.assignment.values(), &[Some(2); 4]);
                        assert_eq!(s.kind, SolverPhaseKind::Exact);
                        assert_eq!(s.phase, 2);
                        println!("job {id}: proven optimum {}", s.score);
                    }),
            )?,
        );
    }
    for job in jobs {
        assert!(job.wait().is_ok());
        assert!(job.callback_failures().is_empty());
    }
    manager.shutdown()?;
    println!("All 4 observed ordered plans reached the proven optimum 8");
    Ok(())
}

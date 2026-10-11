//! Original nonzero end-to-end fact-change control; no upstream method credit.
#[path = "fixtures/live_assembled_fixture.rs"]
mod fixture;
use fixture::Solution;
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
#[test]
fn changed_fact_enters_real_native_domains_and_changes_the_restarted_optimum() {
    let mut manager = LiveSolverManager::<Solution, Solution, String>::new(1).unwrap();
    let (ended, first_ended) = mpsc::channel();
    let scores = Arc::new(Mutex::new(vec![]));
    let results = scores.clone();
    let best = Arc::new(Mutex::new(None::<Solution>));
    let delivered = best.clone();
    let job = manager
        .submit_with_change_executor(
            1,
            Solution::initialized(4, 4),
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            move |p, _, token, _| {
                let factory = fixture::factory_with_objective(p.clone(), true);
                let result = factory
                    .solve_assembled_with(p.clone(), |resource, phase| resource.phase(phase, token))
                    .map_err(|e| format!("{:?}", e.primary))?;
                results.lock().unwrap().push(result.score);
                if p.values.len() == 4 {
                    ended.send(()).unwrap();
                }
                Ok(result.working)
            },
            |operation| {
                let input = operation.previous().unwrap().solution.as_ref().clone();
                assert_eq!(input.assignment, vec![Some(3); 4]);
                let factory = fixture::factory_with_objective(input.clone(), true);
                let result = factory
                    .apply_assembled_problem_change(input, |resource| {
                        assert_eq!(resource.input().assignment, vec![Some(3); 4]);
                        operation.apply().map_err(std::io::Error::other)?;
                        resource.change(|working| {
                            working.values.clone_from(&operation.problem().values)
                        })
                    })
                    .map_err(|e| format!("{:?}", e.primary))?;
                assert_eq!(result.working.assignment, vec![Some(3); 4]);
                assert_eq!(result.score, SimpleScore::new(30));
                *operation.problem_mut() = result.working;
                Ok(())
            },
            LiveSolverCallbacks::<Solution>::new(move |_, event| {
                if event.solution.revision == 1 {
                    *delivered.lock().unwrap() = Some(event.solution.solution.as_ref().clone());
                }
            }),
        )
        .unwrap();
    first_ended.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|p| {
            p.values.push(4);
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    job.cancel();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
    assert_eq!(
        *scores.lock().unwrap(),
        vec![SimpleScore::new(30), SimpleScore::new(40)]
    );
    let best = best.lock().unwrap();
    assert_eq!(best.as_ref().unwrap().assignment, vec![Some(4); 4]);
    assert_eq!(best.as_ref().unwrap().values, vec![0, 1, 2, 3, 4]);
}

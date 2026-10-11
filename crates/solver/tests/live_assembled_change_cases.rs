// Adapted from pinned Community DefaultSolverTest. Apache-2.0; see provenance.
#[path = "fixtures/live_assembled_fixture.rs"]
mod fixture;
use fixture::{Solution, View};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
// Rc views stay on the actual solver worker. They retain closed context slots,
// never scoring resources, and are compared directly instead of identity tokens.
#[derive(Default)]
struct Views {
    first: Vec<View>,
    last: Option<View>,
}
thread_local! { static VIEWS: RefCell<Views> = RefCell::default(); }
#[test]
fn problem_change_after_the_solve_runs_on_a_score_director_of_its_own() {
    let mut manager = LiveSolverManager::<Solution, Solution, String>::new(1).unwrap();
    let (ended, first_ended) = mpsc::channel();
    let (restarted, restarted_phase) = mpsc::channel();
    let observations = Arc::new(Mutex::new((false, None, false, 0usize)));
    let changed = observations.clone();
    let solved = observations.clone();
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
                let factory = fixture::factory(p.clone());
                let mut callback = |event, view: View| {
                    if let FactoryRunEvent::PhaseEntered { index } = event {
                        VIEWS.with(|stored| {
                            let mut stored = stored.borrow_mut();
                            if index == 0 {
                                stored.first.push(view);
                                if stored.first.len() == 2 {
                                    let mut obs = solved.lock().unwrap();
                                    obs.2 = !stored.first[0].same_runtime_view(&stored.first[1]);
                                    obs.3 = stored.first.len();
                                    restarted.send(()).unwrap();
                                }
                            } else if stored.last.is_none() {
                                stored.last = Some(view);
                            }
                        });
                    }
                    Ok(())
                };
                let result = factory
                    .solve_observed_assembled_with(
                        p.clone(),
                        &mut FactoryRunCallbacks {
                            lifecycle: Some(&mut callback),
                            failure: None,
                        },
                        |resource, phase| resource.phase(phase, token),
                    )
                    .map_err(|e| format!("{:?}", e.primary))?;
                if p.values.len() == 4 {
                    VIEWS.with(|v| {
                        let v = v.borrow();
                        assert!(v.last.as_ref().unwrap().runtime_view_closed());
                        assert!(!v.first[0].same_runtime_view(v.last.as_ref().unwrap()));
                    });
                    ended.send(()).unwrap();
                }
                Ok(result.working)
            },
            move |operation| {
                let input = operation
                    .previous()
                    .expect("completed first run")
                    .solution
                    .as_ref()
                    .clone();
                let factory = fixture::factory(input.clone());
                let result = factory
                    .apply_observed_assembled_problem_change(
                        input,
                        |view| {
                            VIEWS.with(|stored| {
                                let stored = stored.borrow();
                                let last = stored.last.as_ref().unwrap();
                                let mut obs = changed.lock().unwrap();
                                obs.0 = last.runtime_view_closed()
                                    && last.read_runtime_view(|_| ()).is_none();
                                assert!(!last.same_runtime_view(&view));
                                assert!(!stored.first[0].same_runtime_view(&view));
                                assert!(!view.runtime_view_closed());
                                obs.1 = Some(
                                    view.read_runtime_view(|resource| resource.input().clone())
                                        .unwrap(),
                                );
                            });
                            Ok(())
                        },
                        |resource| {
                            operation.apply().map_err(std::io::Error::other)?;
                            resource.change(|working| {
                                working.values.clone_from(&operation.problem().values)
                            })?;
                            Ok(())
                        },
                    )
                    .map_err(|e| format!("{:?}", e.primary))?;
                *operation.problem_mut() = result.working;
                Ok(())
            },
            LiveSolverCallbacks::<Solution>::new(move |_, event| {
                if event.solution.revision == 1 && event.solution.solution.values.len() == 5 {
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
    restarted_phase.recv_timeout(WAIT).unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    job.cancel();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
    let obs = observations.lock().unwrap();
    assert!(obs.0);
    let working = obs.1.as_ref().unwrap();
    assert_eq!(working.assignment.len(), 4);
    assert!(working.assignment.iter().all(Option::is_some));
    assert_eq!(working.values.len(), 4);
    assert_eq!(best.lock().unwrap().as_ref().unwrap().values.len(), 5);
    assert_eq!(obs.3, 2);
    assert!(obs.2);
}

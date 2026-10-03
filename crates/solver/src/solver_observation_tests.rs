//! Original controlled-time and retained-state observer checks.
use super::*;
use std::cell::Cell;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn observer_time_preserves_shared_stagnation_in_local_and_exact_phases() {
    for exact_phase in [false, true] {
        let elapsed = Cell::new(0);
        let mut events = Vec::new();
        let mut callback = |u: SolverUpdate<'_, Assignment<usize>, SimpleScore>| {
            events.push((u.phase, u.kind, u.score));
            if u.phase == 1 {
                elapsed.set(10);
            }
        };
        let mut observation = PlanObservation::new(Some(&mut callback));
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap() as i64)
        })
        .unwrap();
        let phases = vec![
            if exact_phase {
                SolverPhase::Exact(ExactPhaseConfig::default())
            } else {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    selection: MoveSelection::FirstAccepted,
                    ..Default::default()
                })
            },
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ];
        let r = run_with_clock(
            SolverLimits {
                max_unimproved_time: Some(ms(5)),
                ..Default::default()
            },
            ConstructionConfig::default(),
            &phases,
            |index, a, request, time| -> Result<_, LocalSearchError> {
                let request = request.with_time(time);
                let kind = request.kind();
                let mut notify = |u: SolutionUpdate<'_, Assignment<usize>, SimpleScore>| {
                    observation.notify(index, kind, u)
                };
                Ok(match request {
                    PhaseRequest::Construction(c) => {
                        constructed!(greedy_construction_with_observer(&p, &n, c, &mut notify)?)
                    }
                    PhaseRequest::LocalSearch(c) => {
                        improved!(crate::solution_observation::local_search_from_observed(
                            time.monitor(),
                            &p,
                            &n,
                            a.unwrap(),
                            c,
                            &mut notify
                        )?)
                    }
                    PhaseRequest::Exact(c, prior) => exact::run_observed(
                        c,
                        prior,
                        time,
                        |visit| crate::search::visit_assignments(&p, visit),
                        |a| n.calculate(&p, a).map(|s| (s, 1)),
                        Some(&mut notify),
                    )?,
                })
            },
            &mut || ms(elapsed.get()),
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            events,
            vec![
                (0, SolverPhaseKind::Construction, SimpleScore::ZERO),
                (
                    1,
                    if exact_phase {
                        SolverPhaseKind::Exact
                    } else {
                        SolverPhaseKind::LocalSearch
                    },
                    SimpleScore::new(1)
                )
            ]
        );
        if !exact_phase {
            assert_eq!(r.work.completed_steps, 1);
        }
    }
}
#[test]
fn exact_observer_elapsed_time_stops_before_next_candidate_with_best_preserved() {
    let elapsed = Cell::new(0);
    let mut events = Vec::new();
    let mut callback = |u: SolverUpdate<'_, Assignment<usize>, SimpleScore>| {
        events.push(u.score);
        if u.phase == 1 {
            elapsed.set(10);
        }
    };
    let mut observation = PlanObservation::new(Some(&mut callback));
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<usize, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap() as i64)
    })
    .unwrap();
    let r = run_with_clock(
        SolverLimits {
            max_time: Some(ms(5)),
            ..Default::default()
        },
        ConstructionConfig::default(),
        &[SolverPhase::Exact(ExactPhaseConfig::default())],
        |index, _, request, time| -> Result<_, LocalSearchError> {
            let request = request.with_time(time);
            let kind = request.kind();
            let mut notify = |u: SolutionUpdate<'_, Assignment<usize>, SimpleScore>| {
                observation.notify(index, kind, u)
            };
            Ok(match request {
                PhaseRequest::Construction(c) => {
                    constructed!(greedy_construction_with_observer(&p, &n, c, &mut notify)?)
                }
                PhaseRequest::Exact(c, prior) => {
                    let start = elapsed.get();
                    exact::run_with_clock_observed(
                        c,
                        prior,
                        time,
                        |visit| crate::search::visit_assignments(&p, visit),
                        |a| n.calculate(&p, a).map(|score| (score, 1)),
                        || ms(elapsed.get() - start),
                        Some(&mut notify),
                    )?
                }
                _ => unreachable!(),
            })
        },
        &mut || ms(elapsed.get()),
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(events, vec![SimpleScore::ZERO, SimpleScore::new(1)]);
}

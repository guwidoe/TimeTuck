//! Original deterministic engine tests using real construction and search.
use super::*;
use std::cell::Cell;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
// Each terminal scoring callback advances one shared simulated elapsed clock.
// Factory preparation advances that same clock before the phase receives a cap.
fn exercise(
    c: SolverConfig<'_>,
    factory_cost: u64,
) -> SolverResult<Assignment<usize>, SimpleScore> {
    exercise_scored(c, factory_cost, |v| v as i64)
}
fn exercise_scored(
    c: SolverConfig<'_>,
    factory_cost: u64,
    score: impl Fn(usize) -> i64,
) -> SolverResult<Assignment<usize>, SimpleScore> {
    let elapsed = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap();
    run_with_clock(
        c.limits,
        c.construction,
        &c.local_search,
        |_, a, request, time| -> Result<_, LocalSearchError> {
            elapsed.set(elapsed.get() + factory_cost);
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<usize>)>();
            b.score("value", s.port(), |(_, v)| {
                elapsed.set(elapsed.get() + 1);
                SimpleScore::new(score(v.unwrap()))
            })
            .unwrap();
            let request = request.with_time(time);
            let start = elapsed.get();
            let clock = || ms(elapsed.get() - start);
            Ok(match request {
                PhaseRequest::Construction(c) => {
                    constructed!(crate::construction::construction_impl_with_clock(
                        &p,
                        c,
                        |a| StreamScoreDirector::new(&p, b.build(), s, a),
                        clock
                    )?)
                }
                PhaseRequest::LocalSearch(c) => {
                    improved!(crate::local_search::search_impl_with_clock_monitored(
                        time.monitor(),
                        &p,
                        a,
                        c,
                        |a| StreamScoreDirector::new(&p, b.build(), s, a),
                        clock
                    )?)
                }
            })
        },
        &mut || ms(elapsed.get()),
    )
    .unwrap()
}
fn plan() -> SolverConfig<'static> {
    SolverConfig {
        local_search: vec![
            LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            };
            3
        ],
        ..Default::default()
    }
}

#[test]
fn one_elapsed_budget_spans_initialization_and_multiple_improvement_phases() {
    // Construction takes 1; improvement phases take 2, 3, 3 callbacks.
    for (limit, evaluations, phases, best) in [
        (0, 1, 1, 0),
        (1, 1, 1, 0),
        (2, 2, 2, 0),
        (3, 3, 2, 1),
        (4, 4, 3, 1),
        (5, 5, 3, 1),
        (6, 6, 3, 2),
        (7, 7, 4, 2),
        (8, 8, 4, 2),
        (9, 9, 4, 2),
        (10, 9, 4, 2),
    ] {
        let r = exercise(
            SolverConfig {
                limits: SolverLimits {
                    max_time: Some(ms(limit)),
                    ..Default::default()
                },
                ..plan()
            },
            0,
        );
        assert_eq!(r.work.evaluations, evaluations, "limit {limit}");
        assert_eq!(r.work.move_evaluations, evaluations - phases);
        assert_eq!(r.phases.len() as u64, phases);
        assert_eq!(r.score, Some(SimpleScore::new(best)));
        assert_eq!(
            r.termination,
            if limit < 10 {
                SolverTermination::TimeLimit
            } else {
                SolverTermination::PlanCompleted
            }
        );
    }
}

#[test]
fn graph_preparation_reduces_remaining_time_before_phase_initialization() {
    for (limit, evaluations, phases, best) in [
        (1, 1, 1, 0),
        (3, 1, 1, 0),
        (4, 2, 2, 0),
        (6, 2, 2, 0),
        (7, 3, 2, 1),
        (8, 4, 3, 1),
        (10, 4, 3, 1),
        (11, 5, 3, 1),
        (12, 6, 3, 2),
    ] {
        let r = exercise(
            SolverConfig {
                limits: SolverLimits {
                    max_time: Some(ms(limit)),
                    ..Default::default()
                },
                ..plan()
            },
            2,
        );
        assert_eq!(r.work.evaluations, evaluations, "limit {limit}");
        assert_eq!(r.phases.len() as u64, phases);
        assert_eq!(r.score, Some(SimpleScore::new(best)));
        assert_eq!(r.termination, SolverTermination::TimeLimit);
    }
}

#[test]
fn phase_local_timing_and_stagnation_do_not_end_a_solve_with_time_remaining() {
    for stagnation in [false, true] {
        let mut c = plan();
        c.limits.max_time = Some(ms(50));
        if stagnation {
            c.local_search[0].max_unimproved_time = Some(ms(0));
        } else {
            c.local_search[0].max_time = Some(ms(1));
        }
        let r = exercise(c, 2);
        assert_eq!(r.phases.len(), 4);
        assert_eq!(r.score, Some(SimpleScore::new(2)));
        assert_eq!(r.termination, SolverTermination::PlanCompleted);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::LocalSearch(if stagnation {
                LocalSearchTermination::UnimprovedTimeLimit
            } else {
                LocalSearchTermination::TimeLimit
            })
        );
    }
    // An equal same-kind cap belongs to the shared deadline.
    let mut c = plan();
    c.limits.max_time = Some(ms(2));
    c.local_search[0].max_time = Some(ms(1));
    let r = exercise(c, 0);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
}

#[test]
fn elapsed_time_precedes_shared_goals_but_preserves_phase_reports() {
    for goal in [false, true] {
        let mut c = plan();
        c.limits.max_time = Some(ms(1));
        if goal {
            c.limits.best_score_limit = Some(SimpleScore::ZERO);
        } else {
            c.limits.stop_when_feasible = true;
        }
        let r = exercise(c, 0);
        assert_eq!(r.termination, SolverTermination::TimeLimit);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
    // A step-limited local phase can finish before its own time poll. The global
    // clock is still checked on return, while the phase's reason stays intact.
    let mut c = plan();
    c.limits.max_time = Some(ms(2));
    c.local_search[0].max_steps = 0;
    let r = exercise(c, 0);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
    );
}

#[test]
fn disabled_shared_clock_is_never_sampled_and_backwards_samples_cannot_add_time() {
    let mut disabled = || panic!("disabled global clock");
    let mut time = SolveTime {
        limit: None,
        clock: &mut disabled,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
    };
    let mut own = Some(ms(5));
    time.bound(&mut own);
    assert_eq!(own, Some(ms(5)));
    assert!(!time.expired());
    assert!(!time.shared_bound);
    let mut samples = [ms(3), ms(1), ms(8), ms(2), ms(10), ms(0)].into_iter();
    let mut clock = || samples.next().unwrap();
    let mut time = SolveTime {
        limit: Some(ms(10)),
        clock: &mut clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
    };
    assert_eq!(time.remaining(), Some(ms(7)));
    assert_eq!(time.remaining(), Some(ms(7)));
    assert_eq!(time.remaining(), Some(ms(2)));
    assert_eq!(time.remaining(), Some(ms(2)));
    assert!(time.expired());
    assert!(time.expired());
}

#[test]
fn solve_stagnation_keeps_improving_prefixes_and_spans_phase_initializations() {
    // Construction and first initialization take one unit each and are excluded.
    // A first improvement is found at 3. The second phase initializes at 4,
    // rejects at 5 and improves at 6. The last phase finishes at 9.
    for (limit, evaluations, phases, best, reason) in [
        (0, 2, 2, 0, SolverTermination::UnimprovedTimeLimit),
        (1, 3, 2, 1, SolverTermination::UnimprovedTimeLimit),
        (2, 5, 3, 1, SolverTermination::UnimprovedTimeLimit),
        (3, 6, 3, 2, SolverTermination::UnimprovedTimeLimit),
        (4, 9, 4, 2, SolverTermination::PlanCompleted),
    ] {
        let mut c = plan();
        c.limits.max_unimproved_time = Some(ms(limit));
        let r = exercise(c, 0);
        assert_eq!(r.work.evaluations, evaluations, "limit {limit}");
        assert_eq!(r.phases.len(), phases, "limit {limit}");
        assert_eq!(r.score, Some(SimpleScore::new(best)));
        assert_eq!(r.termination, reason);
    }
}

#[test]
fn first_preparation_is_excluded_but_later_factories_consume_stagnation_time() {
    for cost in [2, 10_000] {
        let mut c = plan();
        c.limits.max_unimproved_time = Some(ms(3));
        let r = exercise(c, cost);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(r.work.evaluations, 4);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(
            r.phases[2].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::UnimprovedTimeLimit)
        );
    }
}

#[test]
fn phase_stagnation_does_not_replace_the_shared_limit() {
    let mut c = plan();
    c.limits.max_unimproved_time = Some(ms(4));
    c.local_search[0].max_unimproved_time = Some(Duration::ZERO);
    let r = exercise(c, 0);
    assert_eq!(r.phases.len(), 4);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::UnimprovedTimeLimit)
    );
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    let mut c = plan();
    c.limits.max_unimproved_time = Some(ms(1));
    c.local_search[0].max_unimproved_time = Some(ms(100));
    let r = exercise(c, 0);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.score, Some(SimpleScore::new(1)));
}

#[test]
fn shared_stagnation_precedes_goals_and_observes_expiry_on_phase_return() {
    let mut c = plan();
    c.limits.max_unimproved_time = Some(ms(1));
    c.limits.best_score_limit = Some(SimpleScore::new(1));
    let r = exercise(c, 0);
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    let mut c = plan();
    c.limits.max_time = Some(ms(3));
    c.limits.max_unimproved_time = Some(ms(1));
    let r = exercise(c, 0);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    let mut c = plan();
    c.limits.max_unimproved_time = Some(Duration::ZERO);
    c.local_search[0].max_steps = 0;
    let r = exercise(c, 0);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
    );
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
}

#[test]
fn exhausted_stagnation_is_not_reset_by_another_phase_or_a_late_improvement() {
    use crate::time_limit::SearchStagnation;
    let elapsed = Cell::new(ms(10_000));
    let mut clock = || elapsed.get();
    let mut time = SolveTime {
        limit: None,
        clock: &mut clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: Some(ms(10)),
        last_improvement: None,
        unimproved_expired: false,
    };
    assert!(!time.stagnated()); // Preparation is unbounded by stagnation.
    time.start_search();
    elapsed.set(ms(10_005));
    time.record_improvement();
    elapsed.set(ms(10_003));
    time.start_search(); // No phase reset; backwards sample cannot buy time.
    assert!(!time.stagnated());
    elapsed.set(ms(10_014));
    assert!(!time.stagnated());
    elapsed.set(ms(10_015));
    time.record_improvement();
    assert!(time.stagnated());
    elapsed.set(ms(10_001));
    time.start_search();
    time.record_improvement();
    assert!(time.stagnated());
}

#[test]
fn equal_score_moves_and_new_phase_initialization_do_not_reset_shared_stagnation() {
    let mut c = plan();
    c.limits.max_unimproved_time = Some(ms(3));
    for phase in &mut c.local_search {
        phase.acceptance = Acceptance::HillClimbing;
    }
    let r = exercise_scored(c, 0, |_| 0);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert_eq!(r.assignment.values(), &[Some(0)]);
    assert_eq!(r.work.evaluations, 5);
    assert_eq!(r.work.accepted_moves, 2);
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
}

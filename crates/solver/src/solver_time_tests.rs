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
fn exercise_scored<'a>(
    c: impl Into<SolverSequence<'a>>,
    factory_cost: u64,
    score: impl Fn(usize) -> i64,
) -> SolverResult<Assignment<usize>, SimpleScore> {
    exercise_variable(c, factory_cost, score, true)
}
fn exercise_variable<'a>(
    c: impl Into<SolverSequence<'a>>,
    factory_cost: u64,
    score: impl Fn(usize) -> i64,
    initialized: bool,
) -> SolverResult<Assignment<usize>, SimpleScore> {
    let c = c.into();
    let elapsed = Cell::new(0);
    let variable = Variable::new(vec![0usize, 1, 2]);
    let p = FiniteProblem::new(vec![if initialized {
        variable.with_initial(0)
    } else {
        variable
    }])
    .unwrap();
    run_with_clock_observed(
        c.limits,
        &c.phases,
        || InitialState {
            required_unassigned_pin: None,
            movable: true,
            assignment: p.initial_assignment(),
            complete: p.is_complete(&p.initial_assignment()),
        },
        &mut PlanObservation::new(None),
        |index, a, request, time, _| {
            let result: Result<_, LocalSearchError> = (|| {
                elapsed.set(elapsed.get() + factory_cost);
                let mut b = StreamGraphBuilder::new();
                let s = b.source::<usize, (usize, Option<usize>)>();
                b.score("value", s.port(), |(_, v)| {
                    elapsed.set(elapsed.get() + 1);
                    SimpleScore::new(score(v.unwrap_or(0)))
                })
                .unwrap();
                let request = request.with_time(time);
                let start = elapsed.get();
                let clock = || ms(elapsed.get() - start);
                Ok(match request {
                    PhaseRequest::RuinRecreate(_) => unreachable!("fixture has no repair phases"),
                    PhaseRequest::Custom(config, target, feasible) => {
                        let r = crate::custom_phase::run_monitored_with_clock(
                            a.unwrap(),
                            config,
                            |a| p.is_complete(a),
                            |a| StreamScoreDirector::new(&p, b.build(), s, a),
                            |c| {
                                for value in [1, 0, 2] {
                                    c.execute(&Move::Change {
                                        variable: 0,
                                        value: Some(value),
                                    })?;
                                }
                                Ok(())
                            },
                            |_| {},
                            clock,
                            time.monitor(),
                            target,
                            feasible,
                        )
                        .map_err(|e| match e {
                            CustomPhaseError::Scoring(e) => LocalSearchError::from(e),
                            _ => panic!("unexpected command failure"),
                        })?;
                        custom_output(r)
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let mut g = b.build();
                        if matches!(c.strategy, ExactSearchStrategy::BranchAndBound(_)) {
                            bounded::run_with_clock(
                                c,
                                prior,
                                time,
                                || crate::branch_tree::BasicTree::new(&p),
                                |a| exact::score_basic(&p, &mut g, &s, a),
                                None,
                                clock,
                            )
                            .map_err(|e| match e {
                                crate::branch_and_bound::TreeSearchError::Evaluation(e) => {
                                    LocalSearchError::from(e)
                                }
                                e => panic!("unknown-bound fixture: {e:?}"),
                            })?
                        } else {
                            exact::run_with_clock(
                                c,
                                prior,
                                time,
                                |visit| crate::search::visit_assignments(&p, visit),
                                |a| exact::score_basic(&p, &mut g, &s, a),
                                clock,
                            )?
                        }
                    }
                    PhaseRequest::Construction(c) => {
                        constructed!(crate::construction::construction_impl_with_clock_monitored(
                            time.construction_monitor(),
                            &p,
                            a.unwrap_or_else(|| p.initial_assignment()),
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
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
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
    let mut time = SolveControl {
        lifecycle: Default::default(),
        condition: None,
        condition_expired: false,
        limit: None,
        clock: &mut disabled,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: None,
        steps: StepProgress::default(),
        construction_steps: 0,
        started: false,
        steps_expired: false,
    };
    let mut own = Some(ms(5));
    time.bound(&mut own);
    assert_eq!(own, Some(ms(5)));
    assert!(!time.expired());
    assert!(!time.shared_bound);
    let mut samples = [ms(3), ms(1), ms(8), ms(2), ms(10), ms(0)].into_iter();
    let mut clock = || samples.next().unwrap();
    let mut time = SolveControl {
        lifecycle: Default::default(),
        condition: None,
        condition_expired: false,
        limit: Some(ms(10)),
        clock: &mut clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: None,
        steps: StepProgress::default(),
        construction_steps: 0,
        started: false,
        steps_expired: false,
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
    use crate::time_limit::SearchControl;
    let elapsed = Cell::new(ms(10_000));
    let mut clock = || elapsed.get();
    let mut time = SolveControl {
        lifecycle: Default::default(),
        condition: None,
        condition_expired: false,
        limit: None,
        clock: &mut clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: Some(ms(10)),
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: None,
        steps: StepProgress::default(),
        construction_steps: 0,
        started: false,
        steps_expired: false,
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

fn step_control<'a>(limit: u64, clock: &'a mut dyn FnMut() -> Duration) -> SolveControl<'a> {
    SolveControl {
        lifecycle: Default::default(),
        condition: None,
        condition_expired: false,
        limit: None,
        clock,
        last: Duration::ZERO,
        shared_bound: false,
        unimproved_limit: None,
        last_improvement: None,
        unimproved_expired: false,
        unimproved_steps_limit: Some(limit),
        steps: StepProgress::default(),
        construction_steps: 0,
        started: false,
        steps_expired: false,
    }
}

#[test]
fn step_only_control_uses_no_clocks_and_preserves_progress_between_phase_budgets() {
    let mut clock = || panic!("step-only solve must not read a clock");
    let mut control = step_control(2, &mut clock);
    assert!(!control.step_stagnated());
    {
        let mut phase = crate::time_limit::TimeBudget::new(None, None, || panic!("no phase timer"))
            .with_shared(control.monitor());
        phase.start_search();
        phase.record_improvement();
        phase.complete_step(true);
        phase.complete_step(false);
        assert_eq!(phase.stop(), None);
    }
    assert_eq!(control.steps.completed_steps(), 2);
    assert_eq!(control.steps.unimproved_steps(), 1);
    {
        let mut phase = crate::time_limit::TimeBudget::new(None, None, || panic!("no phase timer"))
            .with_shared(control.monitor());
        phase.start_search();
        phase.complete_step(false);
        assert_eq!(
            phase.stop(),
            Some(LocalSearchTermination::UnimprovedStepLimit)
        );
        phase.record_improvement();
        phase.complete_step(true); // Even an invalid late notification cannot revive it.
        assert_eq!(
            phase.stop(),
            Some(LocalSearchTermination::UnimprovedStepLimit)
        );
    }
    assert_eq!(control.steps.completed_steps(), 3);
    assert_eq!(control.steps.unimproved_steps(), 2);
}

#[test]
fn shared_steps_preserve_time_priority_and_phase_return_reasons() {
    let mut c = plan();
    c.limits.max_unimproved_steps = Some(0);
    c.local_search[0].max_unimproved_time = Some(Duration::ZERO);
    let r = exercise(c, 0);
    assert_eq!(r.termination, SolverTermination::UnimprovedStepLimit);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::UnimprovedTimeLimit)
    );
    let mut c = plan();
    c.limits.max_unimproved_steps = Some(0);
    c.limits.max_time = Some(ms(2));
    let r = exercise(c, 0);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    let mut c = plan();
    c.limits.max_unimproved_steps = Some(0);
    c.local_search[0].max_steps = 0;
    let r = exercise(c, 0);
    assert_eq!(r.termination, SolverTermination::UnimprovedStepLimit);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
    );
}

fn exact_plan() -> SolverPlan<'static> {
    SolverPlan {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
        ..Default::default()
    }
}
#[test]
fn exact_shared_deadline_includes_factories_and_final_exhaustion_wins() {
    for factory in [0, 2] {
        for cap in 0..=10 {
            let mut c = exact_plan();
            c.limits.max_time = Some(ms(cap));
            let r = exercise_scored(c, factory, |v| v as i64);
            let construction_end = factory + 1;
            let exact_start = construction_end + factory;
            let candidates = if cap <= construction_end {
                0
            } else {
                cap.saturating_sub(exact_start).min(3)
            };
            assert_eq!(
                r.work.evaluations,
                1 + candidates,
                "factory={factory},cap={cap}"
            );
            assert_eq!(r.work.move_evaluations, candidates);
            assert_eq!(
                r.score,
                Some(SimpleScore::new(candidates.saturating_sub(1) as i64))
            );
            assert_eq!(
                r.termination,
                if candidates == 3 {
                    SolverTermination::Optimal
                } else {
                    SolverTermination::TimeLimit
                }
            );
        }
    }
}
#[test]
fn exact_unimproved_time_resets_only_for_strict_improvement_before_expiry() {
    for increasing in [false, true] {
        for cap in 0..=4 {
            let mut c = exact_plan();
            c.limits.max_unimproved_time = Some(ms(cap));
            let r = exercise_scored(c, 0, |v| if increasing { v as i64 } else { 0 });
            let candidates = cap.min(3);
            assert_eq!(r.work.evaluations, 1 + candidates);
            assert_eq!(
                r.termination,
                if candidates == 3 {
                    SolverTermination::Optimal
                } else {
                    SolverTermination::UnimprovedTimeLimit
                }
            );
            assert_eq!(
                r.score,
                Some(SimpleScore::new(if increasing {
                    candidates.saturating_sub(1) as i64
                } else {
                    0
                }))
            );
        }
    }
    // A bounded first phase improves at t=3; the second phase begins with that
    // same retained best. Its first two candidates tie/worsen, so t=6 expires.
    let mut c = exact_plan();
    c.limits.max_unimproved_time = Some(ms(3));
    c.phases.insert(
        0,
        SolverPhase::Exact(ExactPhaseConfig {
            max_evaluations: 2,
            ..Default::default()
        }),
    );
    let r = exercise_scored(c, 0, |v| v as i64);
    assert_eq!(r.termination, SolverTermination::Optimal); // final improvement at t=6 still proves optimum
    assert_eq!(r.work.evaluations, 6);
    assert_eq!(r.phases.len(), 3);
    // Local-search initialization starts the shared timer before exact handoff.
    let mut c = exact_plan();
    c.limits.max_unimproved_time = Some(ms(3));
    c.phases.insert(
        0,
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 0,
            ..Default::default()
        }),
    );
    let r = exercise_scored(c, 0, |_| 0);
    assert_eq!(r.termination, SolverTermination::Optimal); // exactly three callbacks exhaust the space
    assert_eq!(r.work.evaluations, 5);
    let mut c = exact_plan();
    c.limits.max_unimproved_time = Some(ms(2));
    c.phases.insert(
        0,
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 0,
            ..Default::default()
        }),
    );
    let r = exercise_scored(c, 0, |_| 0);
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.work.evaluations, 4);
}
#[test]
fn exact_improvements_reset_shared_step_stagnation_without_adding_steps() {
    use crate::time_limit::SearchControl;
    let mut clock = || panic!("step-only solve must not sample a clock");
    let mut control = step_control(3, &mut clock);
    control.start_search();
    control.complete_step(false);
    control.complete_step(false);
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2, 3])]).unwrap();
    let out = exact::run_with_clock(
        ExactPhaseConfig {
            max_evaluations: 2,
            ..Default::default()
        },
        Some(SimpleScore::ZERO),
        &mut control,
        |visit| crate::search::visit_assignments(&p, visit),
        |a| Ok::<_, std::convert::Infallible>((SimpleScore::new(a.values()[0].unwrap()), 1)),
        || panic!("no phase timer"),
    )
    .unwrap();
    assert_eq!(out.work.completed_steps, 0);
    assert_eq!(control.steps.completed_steps(), 2);
    assert_eq!(control.steps.unimproved_steps(), 0);
    control.complete_step(false);
    control.complete_step(false);
    assert_eq!(control.stop(), None);
    control.complete_step(false);
    assert_eq!(
        control.stop(),
        Some(LocalSearchTermination::UnimprovedStepLimit)
    );
}

#[test]
fn repeated_construction_shares_deadline_and_does_not_restart_stagnation() {
    let construction = SolverPhase::Construction(ConstructionConfig::default());
    let local = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 0,
        ..Default::default()
    });
    let mut c = SolverPlan {
        phases: vec![construction.clone(), construction.clone(), local.clone()],
        ..Default::default()
    };
    c.limits.max_unimproved_time = Some(ms(0));
    let r = exercise_scored(c.clone(), 2, |v| v as i64);
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.phases.len(), 4); // All three construction phases precede stagnation.
    assert_eq!(r.work.evaluations, 4);
    c.limits.max_unimproved_time = None;
    c.limits.max_time = Some(ms(7));
    let r = exercise_scored(c, 2, |v| v as i64);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.phases.len(), 3); // Third factory exceeds the shared deadline.
    assert_eq!(r.work.evaluations, 3); // Zero remaining phase time permits initialization.
    let r = exercise_scored(
        SolverPlan {
            limits: SolverLimits {
                max_unimproved_time: Some(ms(2)),
                ..Default::default()
            },
            phases: vec![
                construction.clone(),
                local,
                construction.clone(),
                construction,
            ],
            ..Default::default()
        },
        2,
        |v| v as i64,
    );
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.phases.len(), 4); // Post-search construction counts toward stagnation.
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}

#[test]
fn direct_search_deadlines_include_first_factory_but_exclude_initial_score_from_stagnation() {
    for exact in [false, true] {
        let phase = if exact {
            SolverPhase::Exact(ExactPhaseConfig::default())
        } else {
            SolverPhase::LocalSearch(LocalSearchConfig::default())
        };
        let c = SolverSequence {
            limits: SolverLimits {
                max_time: Some(ms(1)),
                ..Default::default()
            },
            phases: vec![phase.clone()],
        };
        let r = exercise_scored(c, 2, |v| v as i64);
        assert_eq!(r.termination, SolverTermination::TimeLimit);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.phases[0].index, 0);
        assert!(r.complete);
        assert_eq!(r.work.evaluations, if exact { 0 } else { 1 });
        assert_eq!(r.score, if exact { None } else { Some(SimpleScore::ZERO) });
        let c = SolverSequence {
            limits: SolverLimits {
                max_unimproved_time: Some(ms(1)),
                ..Default::default()
            },
            phases: vec![phase],
        };
        let r = exercise_scored(c, 100, |_| 0);
        assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(r.work.evaluations, if exact { 1 } else { 2 });
        assert_eq!(r.work.move_evaluations, 1);
        assert!(r.complete);
    }
}

#[test]
fn custom_shared_deadlines_include_factory_initialization_and_candidate_prefixes() {
    for factory in [0, 2] {
        for limit in 0..=12 {
            let phases = vec![SolverPhase::Custom(CustomPhaseConfig::default()); 2];
            let r = exercise_scored(
                SolverSequence {
                    phases,
                    limits: SolverLimits {
                        max_time: Some(ms(limit)),
                        ..Default::default()
                    },
                },
                factory,
                |v| v as i64,
            );
            // Each scored callback costs 1 ms; each entered phase adds factory cost.
            // Initial scoring is permitted even after its factory exhausts the cap.
            let first_scores = limit.saturating_sub(factory).clamp(1, 4);
            let mut expected = first_scores;
            let first_elapsed = factory + first_scores;
            if first_scores == 4 && first_elapsed < limit {
                expected += (limit - first_elapsed).saturating_sub(factory).clamp(1, 4);
            }
            assert_eq!(
                r.work.evaluations, expected,
                "factory={factory} limit={limit}"
            );
            assert!(r.work.evaluations <= 8);
            assert_eq!(
                r.work.move_evaluations,
                r.work.evaluations - r.phases.len() as u64
            );
            assert_eq!(
                r.termination,
                if factory * r.phases.len() as u64 + expected >= limit {
                    SolverTermination::TimeLimit
                } else {
                    SolverTermination::PlanCompleted
                }
            );
        }
    }
}
#[test]
fn custom_stagnation_spans_phases_and_initial_scoring_does_not_reset_it() {
    for limit in 0..=8 {
        let r = exercise_scored(
            SolverSequence {
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default()); 2],
                limits: SolverLimits {
                    max_unimproved_time: Some(ms(limit)),
                    ..Default::default()
                },
            },
            0,
            |_| 0,
        );
        // First initialization starts the timer at t=1. Later initial scoring is
        // included in the elapsed interval and cannot restart it.
        let expected = if limit <= 3 {
            1 + limit
        } else {
            (1 + limit).min(8)
        };
        assert_eq!(r.work.evaluations, expected, "limit={limit}");
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(
            r.termination,
            if limit <= 7 {
                SolverTermination::UnimprovedTimeLimit
            } else {
                SolverTermination::PlanCompleted
            }
        );
    }
}

#[test]
fn bounded_phases_share_deadlines_and_stagnation_through_factory_handoffs() {
    let bounded = || {
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        })
    };
    for limit in [2, 3] {
        let r = exercise_scored(
            SolverSequence {
                phases: vec![bounded()],
                limits: SolverLimits {
                    max_time: Some(ms(limit)),
                    ..Default::default()
                },
            },
            2,
            |v| v as i64,
        );
        assert_eq!(r.termination, SolverTermination::TimeLimit);
        assert_eq!(r.work.evaluations, limit - 2);
        assert_eq!(r.score, None);
    }
    let mut first = bounded();
    let SolverPhase::Exact(ref mut c) = first else {
        unreachable!()
    };
    c.max_time = Some(ms(1));
    let r = exercise_scored(
        SolverSequence {
            phases: vec![first, bounded()],
            limits: SolverLimits {
                max_time: Some(ms(5)),
                ..Default::default()
            },
        },
        2,
        |v| v as i64,
    );
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.phases[0].work.evaluations, 1);
    assert_eq!(r.phases[1].work.evaluations, 0);
    let mut first = bounded();
    let SolverPhase::Exact(ref mut c) = first else {
        unreachable!()
    };
    c.max_evaluations = 2;
    let r = exercise_scored(
        SolverSequence {
            phases: vec![first, bounded()],
            limits: SolverLimits {
                max_unimproved_time: Some(ms(3)),
                ..Default::default()
            },
        },
        2,
        |v| v as i64,
    );
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.score, Some(SimpleScore::new(0))); // First domain value is zero.
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::Exact(ExactPhaseTermination::UnimprovedTimeLimit)
    );
}

#[test]
fn diminished_returns_is_phase_local_and_excludes_factory_preparation() {
    let rate = crate::DiminishedReturnsConfig::new(ms(2), 1, 1).unwrap();
    let c = SolverSequence {
        phases: vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                diminished_returns: Some(rate),
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    };
    // Flat objective forces the local window to expire after real scoring work.
    let short = exercise_scored(c.clone(), 0, |_| 0);
    let expensive = exercise_scored(c, 50_000, |_| 0);
    assert_eq!(short.work, expensive.work);
    for r in [short, expensive] {
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::DiminishedReturns)
        );
        assert!(r.phases[0].work.evaluations > 1);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::Exact(ExactPhaseTermination::Exhausted)
        );
        assert_eq!(r.termination, SolverTermination::Optimal);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}

#[test]
fn significant_improvement_is_phase_local_and_excludes_factory_preparation() {
    let rate = crate::SignificantImprovementConfig::new(ms(2), SimpleScore::new(7)).unwrap();
    let c = SolverSequence {
        phases: vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                significant_improvement: Some(&rate),
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    };
    // Flat objective forces the local window to expire after real scoring work.
    let short = exercise_scored(c.clone(), 0, |_| 0);
    let expensive = exercise_scored(c, 50_000, |_| 0);
    assert_eq!(short.work, expensive.work);
    for r in [short, expensive] {
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::SignificantImprovementTimeLimit)
        );
        assert!(r.phases[0].work.evaluations > 1);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::Exact(ExactPhaseTermination::Exhausted)
        );
        assert_eq!(r.termination, SolverTermination::Optimal);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}

#[test]
fn composed_stagnation_can_reset_until_the_whole_condition_is_reached() {
    use crate::time_limit::SearchControl;
    let elapsed = Cell::new(0);
    let mut clock = || ms(elapsed.get());
    let children = [
        SearchTermination::Condition(SearchLimit::UnimprovedTime(ms(5))),
        SearchTermination::Condition(SearchLimit::Steps(3)),
    ];
    let condition = SearchTermination::All(&children);
    let mut control = step_control(u64::MAX, &mut clock);
    control.condition = Some(&condition);
    control.start_search();
    elapsed.set(6);
    assert_eq!(control.stop(), None); // Time child true, step child false.
    control.record_improvement(); // Reset time; no individual-child latch.
    control.complete_step(false);
    control.complete_step(false);
    control.complete_step(false);
    assert_eq!(control.stop(), None);
    elapsed.set(11);
    assert_eq!(control.stop(), Some(LocalSearchTermination::StepCondition));
    control.record_improvement();
    control.start_search();
    assert_eq!(control.stop(), Some(LocalSearchTermination::StepCondition));
}

#[test]
fn composed_elapsed_limits_stop_inside_exact_and_custom_phases() {
    let children = [
        SearchTermination::Condition(SearchLimit::Time(ms(1))),
        SearchTermination::Condition(SearchLimit::Time(ms(2))),
    ];
    let condition = SearchTermination::All(&children);
    for phase in [
        SolverPhase::Exact(ExactPhaseConfig::default()),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        }),
        SolverPhase::Custom(CustomPhaseConfig::default()),
    ] {
        let r = exercise_scored(
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![phase.clone(), phase],
            },
            0,
            |v| v as i64,
        );
        assert_eq!(r.termination, SolverTermination::Condition);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.work.evaluations, 2);
        assert!(matches!(
            r.phases[0].termination,
            PhaseTermination::Exact(ExactPhaseTermination::Condition)
                | PhaseTermination::Custom(CustomPhaseTermination::Condition)
        ));
    }
}

#[test]
fn composed_limits_interrupt_construction_before_finishing_the_domain() {
    for cap in 1..=3 {
        let children = [
            SearchTermination::Condition(SearchLimit::Time(ms(cap))),
            SearchTermination::Condition(SearchLimit::Time(ms(1))),
        ];
        let condition = SearchTermination::All(&children);
        let r = exercise_variable(
            SolverConfig {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                ..plan()
            },
            0,
            |v| v as i64,
            false,
        );
        assert_eq!(r.termination, SolverTermination::Condition);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.work.evaluations, cap);
        assert_eq!(r.work.move_evaluations, cap - 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Construction(ConstructionTermination::Condition)
        );
        assert_eq!(
            r.assignment.values(),
            &[if cap == 1 {
                None
            } else {
                Some((cap - 2) as usize)
            }]
        );
    }
}

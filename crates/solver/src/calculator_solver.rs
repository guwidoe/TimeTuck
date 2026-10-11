//! Complete calculator-backed plans with shared phase orchestration.
//!
//! These entry points accept the existing SolverSequence/MixedSolverSequence and
//! SolverExtensions types. Construction, local search, ruin-and-recreate, custom
//! commands, enumeration and branch-and-bound use the existing engines, limits,
//! event rules, initialization handling and best-state handoffs.
//!
//! Supply a borrowed deterministic factory for an independent IncrementalCalculator
//! per scoring session. It must support partial assignments. Construction/local
//! phases reuse incremental state for candidate moves; exact nodes and repair
//! reconstructions create fresh sessions. Factory work belongs to cooperative
//! phase/solve timing. Skipped phases, zero budgets and pre-cancellation do not
//! create calculators. Phase validation may still reject a skipped plan.
//!
//! Each calculated score is one opaque constraint-work unit. Callback work and
//! diagnostic recomputation are excluded, as in the standalone calculator APIs.
//! Match collection is enabled only when SolverMetrics requests best constraint
//! totals. Unsupported calculators report MatchingDisabled; other metrics leave
//! matching disabled. Use public calculator directors to inspect justifications.
use super::*;
use crate::solution_observation::Observed;

type MixedProvider<'a, B, L> =
    crate::MoveProviderBinding<'a, MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>;
type MixedProviderContext<'a, B, L, S> = crate::provider_search::ProviderContext<
    'a,
    MixedProblem<B, L>,
    MixedAssignment<B, L>,
    MixedMove<B, L>,
    S,
>;

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_calculator<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_calculator_impl(p, factory, c, None, None, None)
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_calculator_with_observer<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_calculator_with_events<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, Assignment<V>, S>),
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_calculator_with_extensions<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S, ScoringError>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    solve_calculator_impl(
        p,
        factory,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
            extensions.failure,
            extensions.clock,
            extensions.phase_terminations,
            extensions.stateful_termination,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

fn solve_calculator_impl<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, Assignment<V>, S>,
    mut command: CommandCallback<'_, Assignment<V>, Move<V>, S, ScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, FiniteProblem<V>, Assignment<V>, Move<V>>>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_phase_models(&c.phases, p)?;
    validate_basic_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        LocalSearchError::from,
    )?;
    let mut observation = PlanObservation::new(observer);
    let capture = observation.enable_constraint_metrics();
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = p.initial_assignment();
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p,
                &assignment,
            ),
            movable: p.variables().iter().any(|v| !v.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
                let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = basic_repair(
                    p,
                    factory,
                    &capture,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                p.validate(&initial)
                    .map_err(ScoringError::from)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| {
                        CalculatorScoreDirector::new(p, a, factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                    },
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && config.strategy.is_bounded()
            {
                p.validate(a.as_ref().expect("initial state"))
                    .map_err(ScoringError::from)
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: LocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || {
                        crate::branch_tree::BasicTree::for_phase(
                            p,
                            config
                                .strategy
                                .is_completion()
                                .then(|| a.as_ref().expect("phase entry")),
                            config.strategy.branch_order(),
                        )
                    },
                    |a| {
                        CalculatorScoreDirector::new(p, a.clone(), factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                            .and_then(|d| {
                                d.capture_metrics();
                                d.score()
                            })
                            .map(|score| (score, 1_u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, LocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::basic_director(
                                p,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                                |a| {
                                    CalculatorScoreDirector::new(p, a, factory, capture.is_some())
                                        .map(|d| d.with_metric_capture(capture.clone()))
                                },
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(basic_provider_local(
                        p,
                        factory,
                        &capture,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        p.validate(a.as_ref().expect("initial state"))
                            .map_err(ScoringError::from)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_basic_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                CalculatorScoreDirector::new(
                                    p,
                                    a.clone(),
                                    factory,
                                    capture.is_some(),
                                )
                                .map(|d| d.with_metric_capture(capture.clone()))
                                .and_then(|d| {
                                    d.capture_metrics();
                                    d.score()
                                })
                                .map(|score| (score, 1_u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        basic_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            Some(&mut notify),
                        )
                    } else {
                        basic_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            None,
                        )
                    }?),
                    PhaseRequest::LocalSearch(c) => {
                        improved!(if observed {
                            basic_local(
                                time.monitor(),
                                p,
                                factory,
                                &capture,
                                a.expect("handoff"),
                                c,
                                Some(&mut notify),
                                None,
                            )
                        } else {
                            basic_local(
                                time.monitor(),
                                p,
                                factory,
                                &capture,
                                a.expect("handoff"),
                                c,
                                None,
                                None,
                            )
                        }?)
                    }
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

fn basic_construction<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: Assignment<V>,
    config: ConstructionConfig<'_>,
    mut observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
) -> Result<ConstructionResult<V, S>, ScoringError> {
    p.validate(&initial)?;
    crate::construction::construction_impl_monitored(shared, p, initial, config, |a| {
        CalculatorScoreDirector::new(p, a, factory, capture.is_some())
            .map(|d| d.with_metric_capture(capture.clone()))
            .map(|d| {
                Observed::new(d, move |u: SolutionUpdate<'_, Assignment<V>, S>| {
                    if let Some(f) = &mut observer {
                        f(u)
                    }
                })
            })
    })
}
#[allow(clippy::too_many_arguments)] // Mirrors kernel controls plus optional metric capture.
fn basic_local<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: Assignment<V>,
    config: LocalSearchConfig<'_, S>,
    mut observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    provider: Option<crate::MoveProviderBinding<'_, FiniteProblem<V>, Assignment<V>, Move<V>>>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    let controls = config.clone();
    let clock = crate::solver_clock::monitored_clock(
        controls.max_time.is_some()
            || controls.max_unimproved_time.is_some()
            || controls.diminished_returns.is_some()
            || controls.significant_improvement.is_some(),
        &shared,
    );
    crate::local_search::search_impl_with_provider(
        shared,
        p,
        Some(initial),
        config,
        |a| {
            CalculatorScoreDirector::new(p, a, factory, capture.is_some())
                .map(|d| d.with_metric_capture(capture.clone()))
                .map(|d| {
                    Observed::new(d, move |u: SolutionUpdate<'_, Assignment<V>, S>| {
                        if let Some(f) = &mut observer {
                            f(u)
                        }
                    })
                })
        },
        clock,
        provider,
    )
}
fn basic_provider_local<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: Assignment<V>,
    config: LocalSearchConfig<'_, S>,
    context: crate::provider_search::ProviderContext<
        '_,
        FiniteProblem<V>,
        Assignment<V>,
        Move<V>,
        S,
    >,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    basic_local(
        context.control,
        p,
        factory,
        capture,
        initial,
        config,
        context.observer,
        Some(context.provider),
    )
}
fn basic_repair<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &FiniteProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<'_, S>,
    observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    crate::ruin_engine::run(
        p,
        initial,
        config,
        observer,
        shared,
        |p, a, construction, monitor| {
            let r = basic_construction(monitor, p, factory, capture, a, construction, None)
                .map_err(LocalSearchError::from)?;
            Ok(crate::ruin_engine::Repair {
                assignment: r.assignment,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
            })
        },
    )
    .map(|r| r.map(|(assignment, score)| ScoredAssignment { assignment, score }))
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_list_calculator<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_calculator_impl(p, factory, c, None, None, None)
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_list_calculator_with_observer<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_list_calculator_with_events<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, ListAssignment<V>, S>),
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_list_calculator_with_extensions<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    mut extensions: SolverExtensions<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
        ListScoringError,
    >,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    solve_list_calculator_impl(
        p,
        factory,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
            extensions.failure,
            extensions.clock,
            extensions.phase_terminations,
            extensions.stateful_termination,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

fn solve_list_calculator_impl<
    'c,
    V: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    c: impl Into<SolverSequence<'c, S>>,
    observer: PlanObserver<'_, ListAssignment<V>, S>,
    mut command: CommandCallback<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    providers: Option<&mut PhaseMoveProviders<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>>>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                SolverPhase::LocalSearch(_) | SolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_phase_models(&c.phases, p)?;
    validate_list_phase_neighborhoods(
        p,
        &c.phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| ListLocalSearchError::from(ListScoringError::from(e)),
    )?;
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: ListScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    let capture = observation.enable_constraint_metrics();
    observation.repaired_initial = prepared != raw_initial;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: None,
            movable: p.entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &c.phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
                let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = list_repair(
                    p,
                    factory,
                    &capture,
                    a.expect("repair handoff"),
                    config,
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                p.validate(&initial)
                    .map_err(ListScoringError::from)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| {
                        ListCalculatorScoreDirector::new(p, a, factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                    },
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && config.strategy.is_bounded()
            {
                p.validate(a.as_ref().expect("initial state"))
                    .map_err(ListScoringError::from)
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: ListLocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || {
                        crate::branch_tree::ListTree::for_phase(
                            p,
                            config
                                .strategy
                                .is_completion()
                                .then(|| a.as_ref().expect("phase entry")),
                            config.strategy.branch_order(),
                        )
                    },
                    |a| {
                        ListCalculatorScoreDirector::new(p, a.clone(), factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                            .and_then(|d| {
                                d.capture_metrics();
                                d.score()
                            })
                            .map(|score| (score, 1_u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, ListLocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::list_director(
                                p,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                                |a| {
                                    ListCalculatorScoreDirector::new(
                                        p,
                                        a,
                                        factory,
                                        capture.is_some(),
                                    )
                                    .map(|d| d.with_metric_capture(capture.clone()))
                                },
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(list_provider_local(
                        p,
                        factory,
                        &capture,
                        a.expect("provider phase handoff"),
                        config.clone(),
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        p.validate(a.as_ref().expect("initial state"))
                            .map_err(ListScoringError::from)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_list_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                ListCalculatorScoreDirector::new(
                                    p,
                                    a.clone(),
                                    factory,
                                    capture.is_some(),
                                )
                                .map(|d| d.with_metric_capture(capture.clone()))
                                .and_then(|d| {
                                    d.capture_metrics();
                                    d.score()
                                })
                                .map(|score| (score, 1_u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(c) => constructed!(if observed {
                        list_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            Some(&mut notify),
                        )
                    } else {
                        list_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            c,
                            None,
                        )
                    }?),
                    PhaseRequest::LocalSearch(c) => {
                        improved!(if observed {
                            list_local(
                                time.monitor(),
                                p,
                                factory,
                                &capture,
                                a.expect("handoff"),
                                c,
                                Some(&mut notify),
                                None,
                            )
                        } else {
                            list_local(
                                time.monitor(),
                                p,
                                factory,
                                &capture,
                                a.expect("handoff"),
                                c,
                                None,
                                None,
                            )
                        }?)
                    }
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

fn list_construction<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: ListAssignment<V>,
    config: ConstructionConfig<'_>,
    mut observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    p.validate(&initial)?;
    crate::list_construction::construction_impl_monitored(shared, p, initial, config, |a| {
        ListCalculatorScoreDirector::new(p, a, factory, capture.is_some())
            .map(|d| d.with_metric_capture(capture.clone()))
            .map(|d| {
                Observed::new(d, move |u: SolutionUpdate<'_, ListAssignment<V>, S>| {
                    if let Some(f) = &mut observer {
                        f(u)
                    }
                })
            })
    })
}
#[allow(clippy::too_many_arguments)] // Mirrors kernel controls plus optional metric capture.
fn list_local<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<'_, S>,
    mut observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
    provider: Option<
        crate::MoveProviderBinding<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>>,
    >,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    let controls = config.clone();
    let clock = crate::solver_clock::monitored_clock(
        controls.max_time.is_some()
            || controls.max_unimproved_time.is_some()
            || controls.diminished_returns.is_some()
            || controls.significant_improvement.is_some(),
        &shared,
    );
    crate::list_local_search::search_impl_with_provider(
        shared,
        p,
        initial,
        config,
        |a| {
            ListCalculatorScoreDirector::new(p, a, factory, capture.is_some())
                .map(|d| d.with_metric_capture(capture.clone()))
                .map(|d| {
                    Observed::new(d, move |u: SolutionUpdate<'_, ListAssignment<V>, S>| {
                        if let Some(f) = &mut observer {
                            f(u)
                        }
                    })
                })
        },
        clock,
        provider,
    )
}
fn list_provider_local<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<'_, S>,
    context: crate::provider_search::ProviderContext<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
    >,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_local(
        context.control,
        p,
        factory,
        capture,
        initial,
        config,
        context.observer,
        Some(context.provider),
    )
}
fn list_repair<
    V: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>>,
>(
    p: &ListProblem<V>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<'_, S>,
    observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    crate::ruin_engine::run(
        p,
        initial,
        config,
        observer,
        shared,
        |p, a, construction, monitor| {
            let r = list_construction(monitor, p, factory, capture, a, construction, None)
                .map_err(ListLocalSearchError::from)?;
            Ok(crate::ruin_engine::Repair {
                assignment: r.assignment,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
            })
        },
    )
    .map(|r| r.map(|(assignment, score)| ScoredListAssignment { assignment, score }))
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_mixed_calculator<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    c: impl Into<MixedSolverSequence<'c, S>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_calculator_impl(p, factory, c, None, None, None)
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_mixed_calculator_with_observer<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Best(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_mixed_calculator_with_events<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut observer: impl FnMut(SolverEvent<'_, MixedAssignment<B, L>, S>),
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_calculator_impl(
        p,
        factory,
        c,
        Some(PlanCallback::Events(&mut observer)),
        None,
        None,
    )
}

/// Run calculator-backed phases with the same native plan controls and events.
pub fn solve_mixed_calculator_with_extensions<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    c: impl Into<MixedSolverSequence<'c, S>>,
    mut extensions: MixedSolverExtensions<'_, B, L, S, MixedScoringError>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    solve_mixed_calculator_impl(
        p,
        factory,
        c,
        PlanCallback::extensions(
            extensions.events,
            extensions.score_check,
            extensions.lifecycle,
            extensions.metrics,
            extensions.failure,
            extensions.clock,
            extensions.phase_terminations,
            extensions.stateful_termination,
        ),
        extensions.command,
        Some(&mut extensions.move_providers),
    )
}

fn solve_mixed_calculator_impl<
    'c,
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore + 'c,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    c: impl Into<MixedSolverSequence<'c, S>>,
    observer: PlanObserver<'_, MixedAssignment<B, L>, S>,
    mut command: CommandCallback<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    providers: Option<&mut crate::solver_extensions::MixedPhaseProviders<'_, B, L>>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let c = c.into();
    if let Some(providers) = &providers {
        providers.validate(c.phases.iter().map(|p| {
            matches!(
                p,
                MixedSolverPhase::LocalSearch(_) | MixedSolverPhase::Construction(_)
            )
        }))?;
    }
    if command.is_none()
        && let Some(phase) = c
            .phases
            .iter()
            .position(|p| matches!(p, MixedSolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    let raw_initial = p.initial_assignment();
    let prepared =
        p.prepare_assignment(&raw_initial)
            .map_err(|error| SolverError::InitialScoring {
                error: MixedScoringError::from(error).into(),
            })?;
    let mut observation = PlanObservation::new(observer);
    let capture = observation.enable_constraint_metrics();
    observation.repaired_initial = prepared != raw_initial;
    let reject_unassigned_pins = c.limits.reject_unassigned_pins;
    let initial = || {
        let assignment = prepared;
        InitialState {
            required_unassigned_pin: required_pin_startup_issue(
                reject_unassigned_pins,
                p.basic(),
                assignment.basic(),
            ),
            movable: p.basic().variables().iter().any(|v| !v.is_pinned())
                || p.lists().entities().iter().any(|e| !e.is_pinned()),
            complete: p.is_complete(&assignment),
            assignment,
        }
    };
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    validate_phase_models(&phases, p)?;
    validate_basic_phase_neighborhoods(
        p.basic(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Basic(e))),
    )?;
    validate_list_phase_neighborhoods(
        p.lists(),
        &phases,
        |phase| providers.as_ref().is_some_and(|p| p.contains(phase)),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Lists(e))),
    )?;
    let mut providers = crate::solver_extensions::ProviderSolve::new(providers);
    run(
        c.limits,
        &phases,
        initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            if matches!(request, PhaseRequest::RuinRecreate(_)) {
                let PhaseRequest::RuinRecreate(config) = request.with_time(time) else {
                    unreachable!()
                };
                let observed = observation.has_events();
                let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                    observation.notify(index, SolverPhaseKind::RuinRecreate, update)
                };
                let r = mixed_repair(
                    p,
                    factory,
                    &capture,
                    a.expect("repair handoff"),
                    MixedRuinRecreateConfig {
                        search: config,
                        order: c.phases[index].repair().order,
                    },
                    observed.then_some(&mut notify),
                    time.monitor(),
                )
                .map_err(|error| SolverError::Repair {
                    phase: index,
                    error,
                })?;
                return Ok(repaired!(r));
            }

            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Custom(config, target, feasible) = request {
                let initial = a.expect("custom phase handoff");
                p.validate(&initial)
                    .map_err(MixedScoringError::from)
                    .map_err(|e| custom_error(index, CustomPhaseError::Scoring(e)))?;
                let r = crate::custom_phase::run_monitored(
                    initial,
                    config,
                    |a| p.is_complete(a),
                    |a| {
                        MixedCalculatorScoreDirector::new(p, a, factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                    },
                    |context| {
                        if initial_only {
                            Ok(())
                        } else {
                            command.as_mut().expect("validated custom callback")(index, context)
                        }
                    },
                    &mut notify,
                    time.monitor(),
                    target,
                    feasible,
                )
                .map_err(|e| custom_error(index, e))?;
                return Ok(custom_output(r));
            }
            if let PhaseRequest::Exact(config, prior) = &request
                && config.strategy.is_bounded()
            {
                p.validate(a.as_ref().expect("initial state"))
                    .map_err(MixedScoringError::from)
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error: MixedLocalSearchError::from(error),
                    })?;
                return bounded::run(
                    config.clone(),
                    prior.clone(),
                    time,
                    || {
                        crate::branch_tree::MixedTree::for_phase(
                            p,
                            config
                                .strategy
                                .is_completion()
                                .then(|| a.as_ref().expect("phase entry")),
                            config.strategy.branch_order(),
                        )
                    },
                    |a| {
                        MixedCalculatorScoreDirector::new(p, a.clone(), factory, capture.is_some())
                            .map(|d| d.with_metric_capture(capture.clone()))
                            .and_then(|d| {
                                d.capture_metrics();
                                d.score()
                            })
                            .map(|score| (score, 1_u128))
                    },
                    observed.then_some(&mut notify),
                )
                .map_err(|e| bounded::error(index, e));
            }
            let result: Result<_, MixedLocalSearchError> = (|| {
                if let PhaseRequest::Construction(config) = &request {
                    let max_scanned_moves = providers
                        .as_ref()
                        .map_or(100_000, |p| p.max_construction_scans);
                    if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                        let context = crate::provider_search::ProviderContext {
                            provider,
                            control: time.construction_monitor(),
                            observer: observed.then_some(&mut notify),
                        };
                        return Ok(provider_constructed(
                            crate::provider_construction::mixed_director(
                                p,
                                a.expect("construction provider handoff"),
                                provider_construction_config(*config, max_scanned_moves),
                                context,
                                |a| {
                                    MixedCalculatorScoreDirector::new(
                                        p,
                                        a,
                                        factory,
                                        capture.is_some(),
                                    )
                                    .map(|d| d.with_metric_capture(capture.clone()))
                                },
                            )?,
                        ));
                    }
                }
                if let PhaseRequest::LocalSearch(config) = &request
                    && let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index))
                {
                    let context = crate::provider_search::ProviderContext {
                        provider,
                        control: time.monitor(),
                        observer: observed.then_some(&mut notify),
                    };
                    return Ok(improved!(mixed_provider_local(
                        p,
                        factory,
                        &capture,
                        a.expect("provider phase handoff"),
                        MixedLocalSearchConfig {
                            search: config.clone(),
                            ..c.phases[index].local().clone()
                        },
                        context
                    )?));
                }
                Ok(match request {
                    PhaseRequest::Custom(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("handled special phase")
                    }
                    PhaseRequest::Exact(c, prior) => {
                        let completion = matches!(c.strategy, ExactSearchStrategy::Complete);
                        p.validate(a.as_ref().expect("initial state"))
                            .map_err(MixedScoringError::from)?;
                        exact::run_observed(
                            c,
                            prior,
                            time,
                            |visit| {
                                exact::visit_mixed_strategy(
                                    p,
                                    a.as_ref().expect("phase entry"),
                                    completion,
                                    visit,
                                )
                            },
                            |a| {
                                MixedCalculatorScoreDirector::new(
                                    p,
                                    a.clone(),
                                    factory,
                                    capture.is_some(),
                                )
                                .map(|d| d.with_metric_capture(capture.clone()))
                                .and_then(|d| {
                                    d.capture_metrics();
                                    d.score()
                                })
                                .map(|score| (score, 1_u128))
                            },
                            observed.then_some(&mut notify),
                        )?
                    }
                    PhaseRequest::Construction(config) => constructed!(if observed {
                        mixed_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            MixedConstructionConfig {
                                construction: config,
                                ..c.phases[index].construction()
                            },
                            Some(&mut notify),
                        )
                    } else {
                        mixed_construction(
                            time.construction_monitor(),
                            p,
                            factory,
                            &capture,
                            a.unwrap_or_else(|| p.initial_assignment()),
                            MixedConstructionConfig {
                                construction: config,
                                ..c.phases[index].construction()
                            },
                            None,
                        )
                    }?),
                    PhaseRequest::LocalSearch(config) => improved!(if observed {
                        mixed_local(
                            time.monitor(),
                            p,
                            factory,
                            &capture,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                            Some(&mut notify),
                            None,
                        )
                    } else {
                        mixed_local(
                            time.monitor(),
                            p,
                            factory,
                            &capture,
                            a.expect("handoff"),
                            MixedLocalSearchConfig {
                                search: config,
                                ..c.phases[index].local().clone()
                            },
                            None,
                            None,
                        )
                    }?),
                })
            })();
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

fn mixed_construction<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: MixedAssignment<B, L>,
    config: MixedConstructionConfig<'_>,
    mut observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
) -> Result<MixedConstructionResult<B, L, S>, MixedScoringError> {
    p.validate(&initial)?;
    crate::mixed_construction::construction_impl_monitored(shared, p, initial, config, |a| {
        MixedCalculatorScoreDirector::new(p, a, factory, capture.is_some())
            .map(|d| d.with_metric_capture(capture.clone()))
            .map(|d| {
                Observed::new(d, move |u: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                    if let Some(f) = &mut observer {
                        f(u)
                    }
                })
            })
    })
}
#[allow(clippy::too_many_arguments)] // Mirrors kernel controls plus optional metric capture.
fn mixed_local<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<'_, S>,
    mut observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
    provider: Option<MixedProvider<'_, B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    let controls = config.search.clone();
    let clock = crate::solver_clock::monitored_clock(
        controls.max_time.is_some()
            || controls.max_unimproved_time.is_some()
            || controls.diminished_returns.is_some()
            || controls.significant_improvement.is_some(),
        &shared,
    );
    crate::mixed_local_search::search_impl_with_provider(
        shared,
        p,
        initial,
        config,
        |a| {
            MixedCalculatorScoreDirector::new(p, a, factory, capture.is_some())
                .map(|d| d.with_metric_capture(capture.clone()))
                .map(|d| {
                    Observed::new(d, move |u: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                        if let Some(f) = &mut observer {
                            f(u)
                        }
                    })
                })
        },
        clock,
        provider,
    )
}
fn mixed_provider_local<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<'_, S>,
    context: MixedProviderContext<'_, B, L, S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed_local(
        context.control,
        p,
        factory,
        capture,
        initial,
        config,
        context.observer,
        Some(context.provider),
    )
}
fn mixed_repair<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>,
>(
    p: &MixedProblem<B, L>,
    factory: &impl Fn() -> C,
    capture: &Option<crate::solver_metrics::ConstraintMetricCapture<S>>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<'_, S>,
    observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    crate::ruin_engine::run(
        p,
        initial,
        config.search,
        observer,
        shared,
        |p, a, construction, monitor| {
            let r = mixed_construction(
                monitor,
                p,
                factory,
                capture,
                a,
                MixedConstructionConfig {
                    construction,
                    order: config.order,
                },
                None,
            )
            .map_err(MixedLocalSearchError::from)?;
            Ok(crate::ruin_engine::Repair {
                assignment: r.assignment,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
            })
        },
    )
    .map(|r| r.map(|(assignment, score)| ScoredMixedAssignment { assignment, score }))
}

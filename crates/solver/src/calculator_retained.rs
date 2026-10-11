//! Shared solver control over one independently cloneable calculator session.
use super::*;
use std::cell::RefCell;

pub(crate) fn basic<V: Eq + Clone, S: AdditiveScore, C, F>(
    p: &FiniteProblem<V>,
    d: &mut CalculatorScoreDirector<'_, V, C, F>,
    c: SolverSequence<'_, S>,
    mut extensions: SolverExtensions<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S, ScoringError>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>>
where
    C: IncrementalCalculator<Assignment<V>, Score = S, Change = Vec<usize>> + Clone,
    F: Fn() -> C,
{
    let phases = &c.phases;
    extensions.move_providers.validate(phases.iter().map(|p| {
        matches!(
            p,
            SolverPhase::Construction(_) | SolverPhase::LocalSearch(_)
        )
    }))?;
    let mut command = extensions.command;
    if command.is_none()
        && let Some(phase) = phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(phases, p)?;
    validate_basic_phase_neighborhoods(
        p,
        phases,
        |phase| extensions.move_providers.contains(phase),
        LocalSearchError::from,
    )?;
    let raw_initial = d.assignment().clone();
    let prepared = raw_initial.clone();
    let mut observation = PlanObservation::new(PlanCallback::extensions(
        extensions.events,
        extensions.score_check,
        extensions.lifecycle,
        extensions.metrics,
        extensions.failure,
        extensions.clock,
        extensions.phase_terminations,
    ));
    observation.repaired_initial = prepared != raw_initial;
    let initial = InitialState {
        required_unassigned_pin: required_pin_startup_issue(
            c.limits.reject_unassigned_pins,
            p,
            &prepared,
        ),
        complete: p.is_complete(&prepared),
        movable: p.variables().iter().any(|v| !v.is_pinned()),
        assignment: prepared,
    };
    let mut providers =
        crate::solver_extensions::ProviderSolve::new(Some(&mut extensions.move_providers));
    run(
        c.limits.clone(),
        phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Construction(config) = &request {
                let max_scanned_moves = providers
                    .as_ref()
                    .map_or(100_000, |p| p.max_construction_scans);
                if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                    return d
                        .run_provider_construction_monitored(
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            crate::provider_search::ProviderContext {
                                provider,
                                control: time.construction_monitor(),
                                observer: observed.then_some(&mut notify),
                            },
                        )
                        .map(provider_constructed)
                        .map_err(|error| SolverError::Phase {
                            phase: index,
                            error: LocalSearchError::from(error),
                        });
                }
            }
            match request {
                PhaseRequest::Construction(config) => d
                    .run_construction_monitored(
                        time.construction_monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config,
                    )
                    .map(|r| constructed!(r))
                    .map_err(|e| SolverError::Phase {
                        phase: index,
                        error: LocalSearchError::from(e),
                    }),
                PhaseRequest::LocalSearch(config) => d
                    .run_local_search_monitored(
                        time.monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config,
                        providers.as_deref_mut().and_then(|p| p.get(index)),
                    )
                    .map(|r| improved!(r))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error,
                    }),
                PhaseRequest::Custom(config, target, feasible) => {
                    let initial = a.expect("custom handoff");
                    let mut best_state = None;
                    let mut save_best = |director: &CalculatorScoreDirector<'_, V, C, F>,
                                         _complete| {
                        best_state = Some(director.capture_retained());
                    };
                    let r = crate::custom_phase::run_monitored(
                        initial,
                        config,
                        |a| p.is_complete(a),
                        |initial| {
                            let initialization_work = d.prepare_retained_phase(initial)?;
                            if initial_only && c.limits.assert_phase_scores {
                                d.assert_consistent()?;
                            }
                            Ok(crate::retained_stream_phase::RetainedPhase {
                                director: &mut *d,
                                initialization_work,
                                on_best: Some(&mut save_best),
                            })
                        },
                        |context| {
                            if initial_only {
                                Ok(())
                            } else {
                                command.as_mut().expect("validated command")(index, context)
                            }
                        },
                        &mut notify,
                        time.monitor(),
                        target,
                        feasible,
                    )
                    .map_err(|e| custom_error(index, e))?;
                    if let Some(state) = best_state {
                        d.install_retained_state(state);
                    }
                    Ok(custom_output(r))
                }
                PhaseRequest::Exact(config, prior) => {
                    let initial = a.expect("exact handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let evaluate = |a: &Assignment<V>| -> Result<(S, u128), ScoringError> {
                        let mut director = director.borrow_mut();
                        let work = director.prepare_retained_phase(a.clone())?;
                        Ok((director.score()?, work))
                    };
                    let mut best = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let completion = config.strategy.is_completion();
                    let out = if config.strategy.is_bounded() {
                        bounded::run(
                            config,
                            prior,
                            time,
                            || {
                                crate::branch_tree::BasicTree::for_phase(
                                    p,
                                    completion.then_some(&initial),
                                )
                            },
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| bounded::error(index, e))?
                    } else {
                        exact::run_observed(
                            config,
                            prior,
                            time,
                            |visit| exact::visit_basic_strategy(p, &initial, completion, visit),
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| SolverError::Phase {
                            phase: index,
                            error: LocalSearchError::from(e),
                        })?
                    };
                    director.into_inner().install_retained_state(best_state);
                    Ok(out)
                }
                PhaseRequest::RuinRecreate(config) => {
                    let initial = a.expect("repair handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let mut best = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let r = crate::ruin_engine::run(
                        p,
                        initial.clone(),
                        config,
                        Some(&mut best),
                        time.monitor(),
                        |repair, initial, config, monitor| {
                            let mut director = director.borrow_mut();
                            // The repair model supplies temporary pins/ranges to selection;
                            // the original bound model keeps registry and scoring identities.
                            let r = crate::construction::construction_impl_monitored(
                                monitor,
                                repair,
                                initial,
                                config,
                                |initial| {
                                    let initialization_work =
                                        director.prepare_retained_phase(initial)?;
                                    Ok(crate::retained_stream_phase::RetainedPhase {
                                        director: &mut **director,
                                        initialization_work,
                                        on_best: None,
                                    })
                                },
                            )
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
                    .map_err(|error| SolverError::Repair {
                        phase: index,
                        error,
                    })?;
                    director.into_inner().install_retained_state(best_state);
                    Ok(repaired!(r.map(|(assignment, score)| ScoredAssignment {
                        assignment,
                        score
                    })))
                }
            }
        },
    )
}

pub(crate) fn list<V: Eq + Clone, S: AdditiveScore, C, F>(
    p: &ListProblem<V>,
    d: &mut ListCalculatorScoreDirector<'_, V, C, F>,
    c: SolverSequence<'_, S>,
    mut extensions: SolverExtensions<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
        ListScoringError,
    >,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>>
where
    C: IncrementalCalculator<ListAssignment<V>, Score = S, Change = Vec<usize>> + Clone,
    F: Fn() -> C,
{
    let phases = &c.phases;
    extensions.move_providers.validate(phases.iter().map(|p| {
        matches!(
            p,
            SolverPhase::Construction(_) | SolverPhase::LocalSearch(_)
        )
    }))?;
    let mut command = extensions.command;
    if command.is_none()
        && let Some(phase) = phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(phases, p)?;
    validate_list_phase_neighborhoods(
        p,
        phases,
        |phase| extensions.move_providers.contains(phase),
        |e| ListLocalSearchError::from(ListScoringError::from(e)),
    )?;
    let raw_initial = d.assignment().clone();
    let prepared = p
        .prepare_assignment(&raw_initial)
        .map_err(|e| SolverError::InitialScoring {
            error: ListLocalSearchError::from(ListScoringError::from(e)),
        })?;
    let mut observation = PlanObservation::new(PlanCallback::extensions(
        extensions.events,
        extensions.score_check,
        extensions.lifecycle,
        extensions.metrics,
        extensions.failure,
        extensions.clock,
        extensions.phase_terminations,
    ));
    observation.repaired_initial = prepared != raw_initial;
    let initial = InitialState {
        required_unassigned_pin: None,
        complete: p.is_complete(&prepared),
        movable: p.entities().iter().any(|v| !v.is_pinned()),
        assignment: prepared,
    };
    let mut providers =
        crate::solver_extensions::ProviderSolve::new(Some(&mut extensions.move_providers));
    run(
        c.limits.clone(),
        phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Construction(config) = &request {
                let max_scanned_moves = providers
                    .as_ref()
                    .map_or(100_000, |p| p.max_construction_scans);
                if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                    return d
                        .run_provider_construction_monitored(
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            crate::provider_search::ProviderContext {
                                provider,
                                control: time.construction_monitor(),
                                observer: observed.then_some(&mut notify),
                            },
                        )
                        .map(provider_constructed)
                        .map_err(|error| SolverError::Phase {
                            phase: index,
                            error: ListLocalSearchError::from(error),
                        });
                }
            }
            match request {
                PhaseRequest::Construction(config) => d
                    .run_construction_monitored(
                        time.construction_monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config,
                    )
                    .map(|r| constructed!(r))
                    .map_err(|e| SolverError::Phase {
                        phase: index,
                        error: ListLocalSearchError::from(e),
                    }),
                PhaseRequest::LocalSearch(config) => d
                    .run_local_search_monitored(
                        time.monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config,
                        providers.as_deref_mut().and_then(|p| p.get(index)),
                    )
                    .map(|r| improved!(r))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error,
                    }),
                PhaseRequest::Custom(config, target, feasible) => {
                    let initial = a.expect("custom handoff");
                    let mut best_state = None;
                    let mut save_best = |director: &ListCalculatorScoreDirector<'_, V, C, F>,
                                         _complete| {
                        best_state = Some(director.capture_retained());
                    };
                    let r = crate::custom_phase::run_monitored(
                        initial,
                        config,
                        |a| p.is_complete(a),
                        |initial| {
                            let initialization_work = d.prepare_retained_phase(initial)?;
                            if initial_only && c.limits.assert_phase_scores {
                                d.assert_consistent()?;
                            }
                            Ok(crate::retained_stream_phase::RetainedPhase {
                                director: &mut *d,
                                initialization_work,
                                on_best: Some(&mut save_best),
                            })
                        },
                        |context| {
                            if initial_only {
                                Ok(())
                            } else {
                                command.as_mut().expect("validated command")(index, context)
                            }
                        },
                        &mut notify,
                        time.monitor(),
                        target,
                        feasible,
                    )
                    .map_err(|e| custom_error(index, e))?;
                    if let Some(state) = best_state {
                        d.install_retained_state(state);
                    }
                    Ok(custom_output(r))
                }
                PhaseRequest::Exact(config, prior) => {
                    let initial = a.expect("exact handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let evaluate = |a: &ListAssignment<V>| -> Result<(S, u128), ListScoringError> {
                        let mut director = director.borrow_mut();
                        let work = director.prepare_retained_phase(a.clone())?;
                        Ok((director.score()?, work))
                    };
                    let mut best = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let completion = config.strategy.is_completion();
                    let out = if config.strategy.is_bounded() {
                        bounded::run(
                            config,
                            prior,
                            time,
                            || {
                                crate::branch_tree::ListTree::for_phase(
                                    p,
                                    completion.then_some(&initial),
                                )
                            },
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| bounded::error(index, e))?
                    } else {
                        exact::run_observed(
                            config,
                            prior,
                            time,
                            |visit| exact::visit_list_strategy(p, &initial, completion, visit),
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| SolverError::Phase {
                            phase: index,
                            error: ListLocalSearchError::from(e),
                        })?
                    };
                    director.into_inner().install_retained_state(best_state);
                    Ok(out)
                }
                PhaseRequest::RuinRecreate(config) => {
                    let initial = a.expect("repair handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let mut best = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let r = crate::ruin_engine::run(
                        p,
                        initial.clone(),
                        config,
                        Some(&mut best),
                        time.monitor(),
                        |repair, initial, config, monitor| {
                            let mut director = director.borrow_mut();
                            // The repair model supplies temporary pins/ranges to selection;
                            // the original bound model keeps registry and scoring identities.
                            let r = crate::list_construction::construction_impl_monitored(
                                monitor,
                                repair,
                                initial,
                                config,
                                |initial| {
                                    let initialization_work =
                                        director.prepare_retained_phase(initial)?;
                                    Ok(crate::retained_stream_phase::RetainedPhase {
                                        director: &mut **director,
                                        initialization_work,
                                        on_best: None,
                                    })
                                },
                            )
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
                    .map_err(|error| SolverError::Repair {
                        phase: index,
                        error,
                    })?;
                    director.into_inner().install_retained_state(best_state);
                    Ok(repaired!(r.map(|(assignment, score)| {
                        ScoredListAssignment { assignment, score }
                    })))
                }
            }
        },
    )
}

pub(crate) fn mixed<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore, C, F>(
    p: &MixedProblem<B, L>,
    d: &mut MixedCalculatorScoreDirector<'_, B, L, C, F>,
    c: MixedSolverSequence<'_, S>,
    mut extensions: MixedSolverExtensions<'_, B, L, S, MixedScoringError>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>>
where
    C: IncrementalCalculator<MixedAssignment<B, L>, Score = S, Change = MixedCalculatorChange>
        + Clone,
    F: Fn() -> C,
{
    let phases: Vec<_> = c.phases.iter().map(MixedSolverPhase::common).collect();
    extensions.move_providers.validate(phases.iter().map(|p| {
        matches!(
            p,
            SolverPhase::Construction(_) | SolverPhase::LocalSearch(_)
        )
    }))?;
    let mut command = extensions.command;
    if command.is_none()
        && let Some(phase) = phases
            .iter()
            .position(|p| matches!(p, SolverPhase::Custom(_)))
    {
        return Err(SolverError::UnboundCustomPhase { phase });
    }
    validate_repair_candidates(&phases, p)?;
    validate_basic_phase_neighborhoods(
        p.basic(),
        &phases,
        |phase| extensions.move_providers.contains(phase),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Basic(e))),
    )?;
    validate_list_phase_neighborhoods(
        p.lists(),
        &phases,
        |phase| extensions.move_providers.contains(phase),
        |e| MixedLocalSearchError::from(MixedScoringError::from(MixedError::Lists(e))),
    )?;
    let raw_initial = d.assignment().clone();
    let prepared = p
        .prepare_assignment(&raw_initial)
        .map_err(|e| SolverError::InitialScoring {
            error: MixedLocalSearchError::from(MixedScoringError::from(e)),
        })?;
    let mut observation = PlanObservation::new(PlanCallback::extensions(
        extensions.events,
        extensions.score_check,
        extensions.lifecycle,
        extensions.metrics,
        extensions.failure,
        extensions.clock,
        extensions.phase_terminations,
    ));
    observation.repaired_initial = prepared != raw_initial;
    let initial = InitialState {
        required_unassigned_pin: required_pin_startup_issue(
            c.limits.reject_unassigned_pins,
            p.basic(),
            prepared.basic(),
        ),
        complete: p.is_complete(&prepared),
        movable: p.basic().variables().iter().any(|v| !v.is_pinned())
            || p.lists().entities().iter().any(|v| !v.is_pinned()),
        assignment: prepared,
    };
    let mut providers =
        crate::solver_extensions::ProviderSolve::new(Some(&mut extensions.move_providers));
    run(
        c.limits.clone(),
        &phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            providers.start();
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let initial_only = observation.initial_only;
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            if let PhaseRequest::Construction(config) = &request {
                let max_scanned_moves = providers
                    .as_ref()
                    .map_or(100_000, |p| p.max_construction_scans);
                if let Some(provider) = providers.as_deref_mut().and_then(|p| p.get(index)) {
                    return d
                        .run_provider_construction_monitored(
                            a.expect("construction provider handoff"),
                            provider_construction_config(*config, max_scanned_moves),
                            crate::provider_search::ProviderContext {
                                provider,
                                control: time.construction_monitor(),
                                observer: observed.then_some(&mut notify),
                            },
                        )
                        .map(provider_constructed)
                        .map_err(|error| SolverError::Phase {
                            phase: index,
                            error: MixedLocalSearchError::from(error),
                        });
                }
            }
            match request {
                PhaseRequest::Construction(config) => d
                    .run_construction_monitored(
                        time.construction_monitor(),
                        a,
                        observed.then_some(&mut notify),
                        MixedConstructionConfig {
                            construction: config,
                            order: c.phases[index].construction().order,
                        },
                    )
                    .map(|r| constructed!(r))
                    .map_err(|e| SolverError::Phase {
                        phase: index,
                        error: MixedLocalSearchError::from(e),
                    }),
                PhaseRequest::LocalSearch(config) => d
                    .run_local_search_monitored(
                        time.monitor(),
                        a,
                        observed.then_some(&mut notify),
                        MixedLocalSearchConfig {
                            search: config,
                            joint_moves: c.phases[index].local().joint_moves,
                        },
                        providers.as_deref_mut().and_then(|p| p.get(index)),
                    )
                    .map(|r| improved!(r))
                    .map_err(|error| SolverError::Phase {
                        phase: index,
                        error,
                    }),
                PhaseRequest::Custom(config, target, feasible) => {
                    let initial = a.expect("custom handoff");
                    let mut best_state = None;
                    let mut save_best =
                        |director: &MixedCalculatorScoreDirector<'_, B, L, C, F>, _complete| {
                            best_state = Some(director.capture_retained());
                        };
                    let r = crate::custom_phase::run_monitored(
                        initial,
                        config,
                        |a| p.is_complete(a),
                        |initial| {
                            let initialization_work = d.prepare_retained_phase(initial)?;
                            if initial_only && c.limits.assert_phase_scores {
                                d.assert_consistent()?;
                            }
                            Ok(crate::retained_stream_phase::RetainedPhase {
                                director: &mut *d,
                                initialization_work,
                                on_best: Some(&mut save_best),
                            })
                        },
                        |context| {
                            if initial_only {
                                Ok(())
                            } else {
                                command.as_mut().expect("validated command")(index, context)
                            }
                        },
                        &mut notify,
                        time.monitor(),
                        target,
                        feasible,
                    )
                    .map_err(|e| custom_error(index, e))?;
                    if let Some(state) = best_state {
                        d.install_retained_state(state);
                    }
                    Ok(custom_output(r))
                }
                PhaseRequest::Exact(config, prior) => {
                    let initial = a.expect("exact handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let evaluate =
                        |a: &MixedAssignment<B, L>| -> Result<(S, u128), MixedScoringError> {
                            let mut director = director.borrow_mut();
                            let work = director.prepare_retained_phase(a.clone())?;
                            Ok((director.score()?, work))
                        };
                    let mut best = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let completion = config.strategy.is_completion();
                    let out = if config.strategy.is_bounded() {
                        bounded::run(
                            config,
                            prior,
                            time,
                            || {
                                crate::branch_tree::MixedTree::for_phase(
                                    p,
                                    completion.then_some(&initial),
                                )
                            },
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| bounded::error(index, e))?
                    } else {
                        exact::run_observed(
                            config,
                            prior,
                            time,
                            |visit| exact::visit_mixed_strategy(p, &initial, completion, visit),
                            evaluate,
                            Some(&mut best),
                        )
                        .map_err(|e| SolverError::Phase {
                            phase: index,
                            error: MixedLocalSearchError::from(e),
                        })?
                    };
                    director.into_inner().install_retained_state(best_state);
                    Ok(out)
                }
                PhaseRequest::RuinRecreate(config) => {
                    let initial = a.expect("repair handoff");
                    let mut best_state = d.capture_retained();
                    let director = RefCell::new(&mut *d);
                    let mut best = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                        best_state = director.borrow().capture_retained();
                        notify(update);
                    };
                    let r = crate::ruin_engine::run(
                        p,
                        initial.clone(),
                        config,
                        Some(&mut best),
                        time.monitor(),
                        |repair, initial, config, monitor| {
                            let mut director = director.borrow_mut();
                            // The repair model supplies temporary pins/ranges to selection;
                            // the original bound model keeps registry and scoring identities.
                            let r = crate::mixed_construction::construction_impl_monitored(
                                monitor,
                                repair,
                                initial,
                                MixedConstructionConfig {
                                    construction: config,
                                    order: c.phases[index].repair().order,
                                },
                                |initial| {
                                    let initialization_work =
                                        director.prepare_retained_phase(initial)?;
                                    Ok(crate::retained_stream_phase::RetainedPhase {
                                        director: &mut **director,
                                        initialization_work,
                                        on_best: None,
                                    })
                                },
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
                    .map_err(|error| SolverError::Repair {
                        phase: index,
                        error,
                    })?;
                    director.into_inner().install_retained_state(best_state);
                    Ok(repaired!(r.map(|(assignment, score)| {
                        ScoredMixedAssignment { assignment, score }
                    })))
                }
            }
        },
    )
}

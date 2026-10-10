//! Shared solver control for already-bound stream directors.
use super::*;
type EventObserver<'a, A, S> = Option<&'a mut dyn FnMut(SolverEvent<'_, A, S>)>;

pub(crate) fn basic<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    d: &mut StreamScoreDirector<'_, '_, V, S>,
    c: SolverConfig<'_, S>,
    observer: EventObserver<'_, Assignment<V>, S>,
) -> Result<SolverResult<Assignment<V>, S>, SolverError<LocalSearchError>> {
    let phases: Vec<_> = std::iter::once(SolverPhase::Construction(c.construction))
        .chain(c.local_search.iter().cloned().map(SolverPhase::LocalSearch))
        .collect();
    validate_basic_phase_neighborhoods(p, &phases, |_| false, LocalSearchError::from)?;
    let prepared = d.assignment().clone();
    let observation = PlanObservation::new(observer.map(PlanCallback::Events));
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
    run(
        c.limits.clone(),
        &phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, Assignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            let result: Result<PhaseOutput<Assignment<V>, S>, LocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Construction(config) => {
                        constructed!(d.run_construction_monitored(
                            time.construction_monitor(),
                            a,
                            observed.then_some(&mut notify),
                            config
                        )?)
                    }
                    PhaseRequest::LocalSearch(config) => improved!(d.run_local_search_monitored(
                        time.monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config
                    )?),
                    // The controller also uses this kernel for frozen-model initialization
                    // and phase assertions; this configuration has no user custom phases.
                    PhaseRequest::Custom(config, target, feasible) => {
                        let initial = a.expect("initial scoring handoff");
                        let r = crate::custom_phase::run_monitored(
                            initial,
                            config,
                            |a| p.is_complete(a),
                            |initial| {
                                let initialization_work = d.prepare_retained_phase(initial)?;
                                if c.limits.assert_phase_scores {
                                    d.assert_consistent()?;
                                }
                                Ok(crate::retained_stream_phase::RetainedPhase {
                                    director: &mut *d,
                                    initialization_work,
                                    on_best: None,
                                })
                            },
                            |_| Ok(()),
                            &mut notify,
                            time.monitor(),
                            target,
                            feasible,
                        )
                        .map_err(|e| match e {
                            CustomPhaseError::Scoring(e) => LocalSearchError::from(e),
                            _ => unreachable!("no user command or ignored operation"),
                        })?;
                        custom_output(r)
                    }
                    PhaseRequest::Exact(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("SolverConfig contains construction and local search only")
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

pub(crate) fn list<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    d: &mut ListStreamScoreDirector<'_, '_, V, S>,
    c: SolverConfig<'_, S>,
    observer: EventObserver<'_, ListAssignment<V>, S>,
) -> Result<SolverResult<ListAssignment<V>, S>, SolverError<ListLocalSearchError>> {
    let phases: Vec<_> = std::iter::once(SolverPhase::Construction(c.construction))
        .chain(c.local_search.iter().cloned().map(SolverPhase::LocalSearch))
        .collect();
    validate_list_phase_neighborhoods(
        p,
        &phases,
        |_| false,
        |e| ListLocalSearchError::from(ListStreamScoringError::from(e)),
    )?;
    let raw_initial = d.assignment().clone();
    let prepared = (p
        .prepare_assignment(&raw_initial)
        .map_err(ListStreamScoringError::from))
    .map_err(|error| SolverError::InitialScoring {
        error: ListLocalSearchError::from(error),
    })?;
    let mut observation = PlanObservation::new(observer.map(PlanCallback::Events));
    observation.repaired_initial = prepared != raw_initial;
    let initial = InitialState {
        required_unassigned_pin: None,
        complete: p.is_complete(&prepared),
        movable: p.entities().iter().any(|e| !e.is_pinned()),
        assignment: prepared,
    };
    run(
        c.limits.clone(),
        &phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, ListAssignment<V>, S>| {
                observation.notify(index, kind, update)
            };
            let result: Result<PhaseOutput<ListAssignment<V>, S>, ListLocalSearchError> = (|| {
                Ok(match request {
                    PhaseRequest::Construction(config) => {
                        constructed!(d.run_construction_monitored(
                            time.construction_monitor(),
                            a,
                            observed.then_some(&mut notify),
                            config
                        )?)
                    }
                    PhaseRequest::LocalSearch(config) => improved!(d.run_local_search_monitored(
                        time.monitor(),
                        a,
                        observed.then_some(&mut notify),
                        config
                    )?),
                    // The controller also uses this kernel for frozen-model initialization
                    // and phase assertions; this configuration has no user custom phases.
                    PhaseRequest::Custom(config, target, feasible) => {
                        let initial = a.expect("initial scoring handoff");
                        let r = crate::custom_phase::run_monitored(
                            initial,
                            config,
                            |a| p.is_complete(a),
                            |initial| {
                                let initialization_work = d.prepare_retained_phase(initial)?;
                                if c.limits.assert_phase_scores {
                                    d.assert_consistent()?;
                                }
                                Ok(crate::retained_stream_phase::RetainedPhase {
                                    director: &mut *d,
                                    initialization_work,
                                    on_best: None,
                                })
                            },
                            |_| Ok(()),
                            &mut notify,
                            time.monitor(),
                            target,
                            feasible,
                        )
                        .map_err(|e| match e {
                            CustomPhaseError::Scoring(e) => ListLocalSearchError::from(e),
                            _ => unreachable!("no user command or ignored operation"),
                        })?;
                        custom_output(r)
                    }
                    PhaseRequest::Exact(..) | PhaseRequest::RuinRecreate(..) => {
                        unreachable!("SolverConfig contains construction and local search only")
                    }
                })
            })(
            );
            result.map_err(|error| SolverError::Phase {
                phase: index,
                error,
            })
        },
    )
}

pub(crate) fn mixed<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    d: &mut MixedStreamScoreDirector<'_, '_, B, L, S>,
    c: MixedSolverConfig<'_, S>,
    observer: EventObserver<'_, MixedAssignment<B, L>, S>,
) -> Result<SolverResult<MixedAssignment<B, L>, S>, SolverError<MixedLocalSearchError>> {
    let phases: Vec<_> = std::iter::once(SolverPhase::Construction(c.construction.construction))
        .chain(
            c.local_search
                .iter()
                .map(|x| SolverPhase::LocalSearch(x.search.clone())),
        )
        .collect();
    validate_basic_phase_neighborhoods(
        p.basic(),
        &phases,
        |_| false,
        |e| MixedLocalSearchError::from(MixedStreamScoringError::from(MixedError::Basic(e))),
    )?;
    validate_list_phase_neighborhoods(
        p.lists(),
        &phases,
        |_| false,
        |e| MixedLocalSearchError::from(MixedStreamScoringError::from(MixedError::Lists(e))),
    )?;
    let raw_initial = d.assignment().clone();
    let prepared = (p
        .prepare_assignment(&raw_initial)
        .map_err(MixedStreamScoringError::from))
    .map_err(|error| SolverError::InitialScoring {
        error: MixedLocalSearchError::from(error),
    })?;
    let mut observation = PlanObservation::new(observer.map(PlanCallback::Events));
    observation.repaired_initial = prepared != raw_initial;
    let initial = InitialState {
        required_unassigned_pin: required_pin_startup_issue(
            c.limits.reject_unassigned_pins,
            p.basic(),
            prepared.basic(),
        ),
        complete: p.is_complete(&prepared),
        movable: p.basic().variables().iter().any(|v| !v.is_pinned())
            || p.lists().entities().iter().any(|e| !e.is_pinned()),
        assignment: prepared,
    };
    run(
        c.limits.clone(),
        &phases,
        || initial,
        observation,
        |index, a, request, time, observation| {
            let request = request.with_time(time);
            let observed = observation.has_events();
            let kind = request.kind();
            let mut notify = |update: SolutionUpdate<'_, MixedAssignment<B, L>, S>| {
                observation.notify(index, kind, update)
            };
            let result: Result<PhaseOutput<MixedAssignment<B, L>, S>, MixedLocalSearchError> =
                (|| {
                    Ok(match request {
                        PhaseRequest::Construction(config) => {
                            constructed!(d.run_construction_monitored(
                                time.construction_monitor(),
                                a,
                                observed.then_some(&mut notify),
                                MixedConstructionConfig {
                                    construction: config,
                                    order: c.construction.order
                                }
                            )?)
                        }
                        PhaseRequest::LocalSearch(config) => {
                            improved!(d.run_local_search_monitored(
                                time.monitor(),
                                a,
                                observed.then_some(&mut notify),
                                MixedLocalSearchConfig {
                                    search: config,
                                    joint_moves: c.local_search[index - 1].joint_moves
                                }
                            )?)
                        }
                        // The controller also uses this kernel for frozen-model initialization
                        // and phase assertions; this configuration has no user custom phases.
                        PhaseRequest::Custom(config, target, feasible) => {
                            let initial = a.expect("initial scoring handoff");
                            let r = crate::custom_phase::run_monitored(
                                initial,
                                config,
                                |a| p.is_complete(a),
                                |initial| {
                                    let initialization_work = d.prepare_retained_phase(initial)?;
                                    if c.limits.assert_phase_scores {
                                        d.assert_consistent()?;
                                    }
                                    Ok(crate::retained_stream_phase::RetainedPhase {
                                        director: &mut *d,
                                        initialization_work,
                                        on_best: None,
                                    })
                                },
                                |_| Ok(()),
                                &mut notify,
                                time.monitor(),
                                target,
                                feasible,
                            )
                            .map_err(|e| match e {
                                CustomPhaseError::Scoring(e) => MixedLocalSearchError::from(e),
                                _ => unreachable!("no user command or ignored operation"),
                            })?;
                            custom_output(r)
                        }
                        PhaseRequest::Exact(..) | PhaseRequest::RuinRecreate(..) => {
                            unreachable!("SolverConfig contains construction and local search only")
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

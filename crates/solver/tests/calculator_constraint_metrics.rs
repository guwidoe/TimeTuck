//! Original end-to-end constraint-total monitoring regressions; no upstream credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[derive(Default)]
struct Counts {
    factories: Cell<u64>,
    resets: Cell<u64>,
    scores: Cell<u64>,
    clones: Cell<u64>,
}
// Deliberately not Clone: snapshots share immutable match records, not justifications.
struct Justification;
struct Cache<A> {
    parts: Vec<i64>,
    total: i64,
    handles: Vec<Option<MatchRegistration<SimpleScore, Justification>>>,
    counts: Rc<Counts>,
    _model: std::marker::PhantomData<A>,
}
impl<A> Clone for Cache<A> {
    fn clone(&self) -> Self {
        self.counts.clones.set(self.counts.clones.get() + 1);
        Self {
            parts: self.parts.clone(),
            total: self.total,
            handles: self.handles.clone(),
            counts: self.counts.clone(),
            _model: Default::default(),
        }
    }
}
impl<A> Cache<A> {
    fn new(counts: &Rc<Counts>) -> Self {
        counts.factories.set(counts.factories.get() + 1);
        Self {
            parts: vec![],
            total: 0,
            handles: vec![],
            counts: counts.clone(),
            _model: Default::default(),
        }
    }
}
trait Parts {
    type Change;
    fn parts(&self) -> Vec<i64>;
    fn touched(&self, change: &Self::Change) -> Vec<usize>;
}
impl Parts for Assignment<i64> {
    type Change = Vec<usize>;
    fn parts(&self) -> Vec<i64> {
        self.values().iter().map(|v| v.unwrap_or(0)).collect()
    }
    fn touched(&self, change: &Self::Change) -> Vec<usize> {
        change.clone()
    }
}
impl Parts for ListAssignment<i64> {
    type Change = Vec<usize>;
    fn parts(&self) -> Vec<i64> {
        self.lists()
            .iter()
            .map(|l| l.iter().enumerate().map(|(i, v)| -(i as i64 + 1) * v).sum())
            .collect()
    }
    fn touched(&self, change: &Self::Change) -> Vec<usize> {
        change.clone()
    }
}
impl Parts for MixedAssignment<i64, i64> {
    type Change = MixedCalculatorChange;
    fn parts(&self) -> Vec<i64> {
        self.basic()
            .parts()
            .into_iter()
            .chain(self.lists().parts())
            .collect()
    }
    fn touched(&self, change: &Self::Change) -> Vec<usize> {
        change
            .basic_variables
            .iter()
            .copied()
            .chain(
                change
                    .list_entities
                    .iter()
                    .map(|i| i + self.basic().values().len()),
            )
            .collect()
    }
}
impl<A: Parts> IncrementalCalculator<A> for Cache<A> {
    type Score = SimpleScore;
    type Justification = Justification;
    type Change = A::Change;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &A,
        r: &mut MatchRegistry<SimpleScore, Justification>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.resets.set(self.counts.resets.get() + 1);
        self.parts = a.parts();
        self.total = self.parts.iter().sum();
        self.handles.clear();
        for (i, part) in self.parts.iter().enumerate() {
            self.handles.push(if r.is_enabled() {
                Some(r.register(i.to_string(), SimpleScore::new(*part), Justification)?)
            } else {
                None
            });
        }
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &A,
        c: &A::Change,
        r: &mut MatchRegistry<SimpleScore, Justification>,
    ) -> Result<(), MatchRegistryError> {
        for i in a.touched(c) {
            self.total -= self.parts[i];
            if let Some(h) = self.handles[i].take() {
                h.cancel(r)?;
            }
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        c: &A::Change,
        r: &mut MatchRegistry<SimpleScore, Justification>,
    ) -> Result<(), MatchRegistryError> {
        let parts = a.parts();
        for i in a.touched(c) {
            self.parts[i] = parts[i];
            self.total += parts[i];
            self.handles[i] = if r.is_enabled() {
                Some(r.register(i.to_string(), SimpleScore::new(parts[i]), Justification)?)
            } else {
                None
            };
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.counts.scores.set(self.counts.scores.get() + 1);
        SimpleScore::new(self.total)
    }
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![]); 2]).unwrap()
}
fn mixed(c: SolverSequence<'_>) -> MixedSolverSequence<'_> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Construction(c) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction: c,
                        ..Default::default()
                    })
                }
                SolverPhase::LocalSearch(c) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: c,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(c) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search: c,
                        ..Default::default()
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    }
}
fn verify<A: Parts>(snapshot: &SolveMetricSnapshot<SimpleScore>, assignment: &A) {
    let BestConstraintTotals::Available(totals) = &snapshot.best_constraint_totals else {
        panic!(
            "expected available totals: {:?}",
            snapshot.best_constraint_totals
        );
    };
    let parts = assignment.parts();
    assert_eq!(totals.len(), parts.len());
    for (i, part) in parts.iter().enumerate() {
        assert_eq!(totals[&i.to_string()].score, SimpleScore::new(*part));
        assert_eq!(totals[&i.to_string()].match_count, 1);
    }
}
fn observe<A: Parts>(metrics: &SolverMetrics<SimpleScore>, event: SolverEvent<'_, A, SimpleScore>) {
    if let SolverEvent::Best(update) = event {
        let snapshots = metrics.snapshot();
        let snapshot = &snapshots.active[0];
        assert_eq!(snapshot.best_score, Some(update.score));
        assert_eq!(snapshot.complete, update.complete);
        verify(snapshot, update.assignment);
    }
}
fn plans() -> Vec<SolverSequence<'static>> {
    let construction = SolverPhase::Construction(ConstructionConfig::default());
    let local = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 3,
        ..Default::default()
    });
    let repair = SolverPhase::RuinRecreate(RuinRecreateConfig {
        minimum_ruined: 1,
        maximum_ruined: 1,
        max_attempts: 3,
        seed: 7,
        ..Default::default()
    });
    let exact = SolverPhase::Exact(ExactPhaseConfig::default());
    let bounded = SolverPhase::Exact(ExactPhaseConfig {
        strategy: ExactSearchStrategy::BranchAndBound(Default::default()),
        ..Default::default()
    });
    let completion = SolverPhase::Exact(ExactPhaseConfig {
        strategy: ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
        ..Default::default()
    });
    vec![
        vec![construction.clone()],
        vec![construction.clone(), local.clone()],
        vec![construction.clone(), repair.clone()],
        vec![exact.clone()],
        vec![bounded.clone()],
        vec![construction.clone(), completion.clone()],
        vec![construction, local, repair, exact, bounded, completion],
    ]
    .into_iter()
    .map(|phases| SolverSequence {
        phases,
        ..Default::default()
    })
    .collect()
}
macro_rules! model_matrix {
    ($test:ident, $problem:expr, $assignment:ty, $director:ident, $solve:ident, $convert:expr) => {
        #[test]
        fn $test() {
            for plan in plans() {
                for retained in [false, true] {
                    let mut baseline = None;
                    for requested in [false, true] {
                        let p = $problem;
                        let counts = Rc::new(Counts::default());
                        let factory = || Cache::<$assignment>::new(&counts);
                        let metrics = SolverMetrics::default();
                        let metrics = if requested {
                            metrics.with_best_constraint_totals()
                        } else {
                            metrics
                        };
                        let mut events = |event: SolverEvent<'_, $assignment, SimpleScore>| {
                            if requested {
                                observe(&metrics, event);
                            }
                        };
                        let extensions = SolverExtensions {
                            metrics: Some(&metrics),
                            events: Some(&mut events),
                            ..Default::default()
                        };
                        let config = ($convert)(plan.clone());
                        let result = if retained {
                            let mut d =
                                $director::new(&p, p.initial_assignment(), factory, requested)
                                    .unwrap();
                            let r = d.run_solver_with_extensions(config, extensions).unwrap();
                            assert_eq!(d.assignment(), &r.assignment);
                            assert_eq!(counts.factories.get(), 1);
                            r
                        } else {
                            $solve(&p, &factory, config, extensions).unwrap()
                        };
                        let latest = metrics.snapshot().latest.unwrap();
                        assert_eq!(latest.best_score, result.score);
                        assert_eq!(latest.work, result.work);
                        if requested {
                            verify(&latest, &result.assignment);
                        } else {
                            assert_eq!(
                                latest.best_constraint_totals,
                                BestConstraintTotals::Disabled
                            );
                        }
                        let work = (
                            result,
                            counts.factories.get(),
                            counts.resets.get(),
                            counts.scores.get(),
                            counts.clones.get(),
                        );
                        if let Some(baseline) = baseline.take() {
                            assert_eq!(work, baseline);
                        } else {
                            baseline = Some(work);
                        }
                    }
                }
            }
        }
    };
}
model_matrix!(
    basic_routes,
    basic(),
    Assignment<i64>,
    CalculatorScoreDirector,
    solve_calculator_with_extensions,
    |c| c
);
model_matrix!(
    list_routes,
    lists(),
    ListAssignment<i64>,
    ListCalculatorScoreDirector,
    solve_list_calculator_with_extensions,
    |c| c
);
model_matrix!(mixed_routes, MixedProblem::new(basic(), lists()), MixedAssignment<i64, i64>, MixedCalculatorScoreDirector, solve_mixed_calculator_with_extensions, mixed);

#[test]
fn custom_best_totals_survive_rejected_and_accepted_worsening_moves() {
    for retained in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
        let counts = Rc::new(Counts::default());
        let factory = || Cache::<Assignment<i64>>::new(&counts);
        let metrics = SolverMetrics::default().with_best_constraint_totals();
        let mut events =
            |event: SolverEvent<'_, Assignment<i64>, SimpleScore>| observe(&metrics, event);
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            ScoringError,
        >| {
            c.execute(&Move::Change {
                variable: 0,
                value: Some(2),
            })?;
            c.execute_if(
                &Move::Change {
                    variable: 1,
                    value: Some(2),
                },
                |_, _| false,
            )?;
            c.execute(&Move::Change {
                variable: 0,
                value: Some(1),
            })?;
            Ok(())
        };
        let config = SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default())],
            ..Default::default()
        };
        let extensions = SolverExtensions {
            command: Some(&mut command),
            events: Some(&mut events),
            metrics: Some(&metrics),
            ..Default::default()
        };
        let result = if retained {
            let mut d =
                CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
            let r = d.run_solver_with_extensions(config, extensions).unwrap();
            assert_eq!(d.assignment(), &r.assignment);
            r
        } else {
            solve_calculator_with_extensions(&p, &factory, config, extensions).unwrap()
        };
        assert_eq!(result.score, Some(SimpleScore::new(2)));
        verify(&metrics.snapshot().latest.unwrap(), &result.assignment);
        assert_eq!(counts.factories.get(), 1);
        assert_eq!(counts.resets.get(), 1);
        assert_eq!(counts.scores.get(), 4);
    }
}

#[test]
fn unavailable_states_do_not_claim_empty_totals() {
    let p = basic();
    let counts = Rc::new(Counts::default());
    let factory = || Cache::<Assignment<i64>>::new(&counts);
    let metrics = SolverMetrics::default().with_best_constraint_totals();
    let mut d = CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, false).unwrap();
    d.run_solver_with_extensions(
        SolverConfig::default(),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        metrics.snapshot().latest.unwrap().best_constraint_totals,
        BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::MatchingDisabled)
    );
    let config = SolverSequence {
        limits: SolverLimits {
            max_evaluations: 0,
            ..Default::default()
        },
        phases: vec![SolverPhase::Construction(Default::default())],
    };
    solve_calculator_with_extensions(
        &p,
        &factory,
        config,
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        metrics.snapshot().latest.unwrap().best_constraint_totals,
        BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::NoBestYet)
    );
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    solve_with_extensions(
        &p,
        &n,
        SolverConfig::default(),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        metrics.snapshot().latest.unwrap().best_constraint_totals,
        BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::UnsupportedBackend)
    );
}
#[derive(Clone)]
struct Empty(bool);
impl IncrementalCalculator<Assignment<i64>> for Empty {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn supports_matches(&self) -> bool {
        self.0
    }
    fn reset(
        &mut self,
        _: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &Assignment<i64>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        _: &Assignment<i64>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
#[test]
fn supported_empty_registry_differs_from_unsupported_calculator_and_director_stays_send_sync() {
    fn send_sync<T: Send + Sync>(_: &T) {}
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
    for supported in [false, true] {
        let metrics = SolverMetrics::default().with_best_constraint_totals();
        let factory = || Empty(supported);
        let d =
            CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, supported).unwrap();
        send_sync(&d);
        send_sync(&metrics);
        solve_calculator_with_extensions(
            &p,
            &factory,
            SolverConfig::default(),
            SolverExtensions {
                metrics: Some(&metrics),
                ..Default::default()
            },
        )
        .unwrap();
        let totals = metrics.snapshot().latest.unwrap().best_constraint_totals;
        if supported {
            assert!(matches!(totals, BestConstraintTotals::Available(ref m) if m.is_empty()));
        } else {
            assert_eq!(
                totals,
                BestConstraintTotals::Unavailable(ConstraintTotalUnavailable::MatchingDisabled)
            );
        }
    }
}

#[test]
fn panic_and_error_close_metrics_and_release_retained_tracker_for_reuse() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for panic in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
        let counts = Rc::new(Counts::default());
        let mut d =
            CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
                .unwrap();
        let metrics = SolverMetrics::default().with_best_constraint_totals();
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            ScoringError,
        >| {
            assert!(!panic, "command panic");
            c.execute(&Move::Change {
                variable: 99,
                value: Some(1),
            })?;
            Ok(())
        };
        let r = catch_unwind(AssertUnwindSafe(|| {
            d.run_solver_with_extensions(
                SolverSequence {
                    phases: vec![SolverPhase::Custom(Default::default())],
                    ..Default::default()
                },
                SolverExtensions {
                    metrics: Some(&metrics),
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
        }));
        if panic {
            assert!(r.is_err());
        } else {
            assert!(r.unwrap().is_err());
        }
        let old = metrics.snapshot();
        assert!(old.active.is_empty());
        assert_eq!(old.failed_runs, 1);
        verify(old.latest.as_ref().unwrap(), d.assignment());
        let r = d
            .run_solver_with_extensions(
                SolverSequence {
                    phases: vec![SolverPhase::Exact(Default::default())],
                    ..Default::default()
                },
                SolverExtensions {
                    metrics: Some(&metrics),
                    ..Default::default()
                },
            )
            .unwrap();
        verify(&metrics.snapshot().latest.unwrap(), &r.assignment);
        assert_eq!(old.latest.unwrap().best_score, Some(SimpleScore::ZERO));
    }
}

macro_rules! provider_case {
    ($test:ident,$problem:expr,$assignment:ty,$director:ident,$solve:ident,$convert:expr,$provider:expr) => {
        #[test]
        fn $test() {
            for retained in [false, true] {
                let p = $problem;
                let counts = Rc::new(Counts::default());
                let factory = || Cache::<$assignment>::new(&counts);
                let metrics = SolverMetrics::default().with_best_constraint_totals();
                let mut provider = $provider;
                let mut providers = PhaseMoveProviders::default();
                providers.bind_shared([0, 1], &mut provider).unwrap();
                let mut events =
                    |event: SolverEvent<'_, $assignment, SimpleScore>| observe(&metrics, event);
                let config = ($convert)(SolverSequence {
                    phases: vec![
                        SolverPhase::Construction(Default::default()),
                        SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 2,
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                });
                let extensions = SolverExtensions {
                    move_providers: providers,
                    metrics: Some(&metrics),
                    events: Some(&mut events),
                    ..Default::default()
                };
                let result = if retained {
                    let mut d = $director::new(&p, p.initial_assignment(), factory, true).unwrap();
                    d.run_solver_with_extensions(config, extensions).unwrap()
                } else {
                    $solve(&p, &factory, config, extensions).unwrap()
                };
                assert!(result.complete);
                verify(&metrics.snapshot().latest.unwrap(), &result.assignment);
            }
        }
    };
}
provider_case!(
    basic_provider_routes,
    basic(),
    Assignment<i64>,
    CalculatorScoreDirector,
    solve_calculator_with_extensions,
    |c| c,
    |p: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| change_moves(p, a)
        .unwrap()
        .collect::<Vec<_>>()
);
provider_case!(
    list_provider_routes,
    lists(),
    ListAssignment<i64>,
    ListCalculatorScoreDirector,
    solve_list_calculator_with_extensions,
    |c| c,
    |p: &ListProblem<i64>, a: &ListAssignment<i64>, _: u64| list_candidates(p, a)
);
provider_case!(mixed_provider_routes,MixedProblem::new(basic(),lists()),MixedAssignment<i64,i64>,MixedCalculatorScoreDirector,solve_mixed_calculator_with_extensions,mixed, |p: &MixedProblem<i64,i64>, a: &MixedAssignment<i64,i64>, _: u64| mixed_candidates(p,a));
fn assigned_lists() -> ListProblem<i64> {
    ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn relocate(reverse: bool) -> ListMove<i64> {
    let (source, destination) = (
        ListPosition {
            entity: 0,
            index: 1,
        },
        ListPosition {
            entity: 1,
            index: 0,
        },
    );
    ListMove::Change {
        source: if reverse { destination } else { source },
        destination: if reverse { source } else { destination },
    }
}
macro_rules! custom_case {
    ($test:ident,$problem:expr,$assignment:ty,$movement:ty,$error:ty,$director:ident,$solve:ident,$convert:expr,$movement_fn:expr) => {
        #[test]
        fn $test() {
            for retained in [false, true] {
                let p = $problem;
                let counts = Rc::new(Counts::default());
                let factory = || Cache::<$assignment>::new(&counts);
                let metrics = SolverMetrics::default().with_best_constraint_totals();
                let mut events =
                    |event: SolverEvent<'_, $assignment, SimpleScore>| observe(&metrics, event);
                let mut command = |_,
                                   c: &mut CustomPhaseContext<
                    '_,
                    $assignment,
                    $movement,
                    SimpleScore,
                    $error,
                >| {
                    c.execute(&($movement_fn)(false))?;
                    c.execute(&($movement_fn)(true))?;
                    Ok(())
                };
                let config = ($convert)(SolverSequence {
                    phases: vec![SolverPhase::Custom(Default::default())],
                    ..Default::default()
                });
                let extensions = SolverExtensions {
                    command: Some(&mut command),
                    metrics: Some(&metrics),
                    events: Some(&mut events),
                    ..Default::default()
                };
                let result = if retained {
                    let mut d = $director::new(&p, p.initial_assignment(), factory, true).unwrap();
                    d.run_solver_with_extensions(config, extensions).unwrap()
                } else {
                    $solve(&p, &factory, config, extensions).unwrap()
                };
                assert_eq!(result.score, Some(SimpleScore::new(-3)));
                verify(&metrics.snapshot().latest.unwrap(), &result.assignment);
                assert_eq!(
                    (
                        counts.factories.get(),
                        counts.resets.get(),
                        counts.scores.get()
                    ),
                    (1, 1, 3)
                );
            }
        }
    };
}
custom_case!(
    list_custom_restoration,
    assigned_lists(),
    ListAssignment<i64>,
    ListMove<i64>,
    ListScoringError,
    ListCalculatorScoreDirector,
    solve_list_calculator_with_extensions,
    |c| c,
    relocate
);
custom_case!(mixed_custom_restoration,MixedProblem::new(FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap(),assigned_lists()),MixedAssignment<i64,i64>,MixedMove<i64,i64>,MixedScoringError,MixedCalculatorScoreDirector,solve_mixed_calculator_with_extensions,mixed,|reverse| MixedMove { basic: Move::Composite(vec![]),lists: relocate(reverse) });

#[derive(Clone)]
struct Balance;
impl IncrementalCalculator<Assignment<i64>> for Balance {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &Assignment<i64>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        if r.is_enabled() {
            let value = a.values()[0].unwrap_or(0);
            r.register("positive", SimpleScore::new(value), ())?;
            r.register("negative", SimpleScore::new(-value), ())?;
        }
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &Assignment<i64>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i64>,
        _: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        r.reset();
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
#[test]
fn construction_final_tie_publishes_final_assignment_contributions() {
    for retained in [false, true] {
        let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned()]).unwrap();
        let metrics = SolverMetrics::default().with_best_constraint_totals();
        let mut best_events = 0;
        let mut events = |e: SolverEvent<'_, Assignment<i64>, SimpleScore>| {
            if let SolverEvent::Best(_) = e {
                best_events += 1;
                let BestConstraintTotals::Available(totals) =
                    &metrics.snapshot().active[0].best_constraint_totals
                else {
                    panic!()
                };
                assert_eq!(totals["positive"].score, SimpleScore::ZERO);
            }
        };
        let config = SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                ..Default::default()
            })],
            ..Default::default()
        };
        let extensions = SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            ..Default::default()
        };
        let result = if retained {
            let mut d =
                CalculatorScoreDirector::new(&p, p.initial_assignment(), || Balance, true).unwrap();
            d.run_solver_with_extensions(config, extensions).unwrap()
        } else {
            solve_calculator_with_extensions(&p, &|| Balance, config, extensions).unwrap()
        };
        assert_eq!(best_events, 1);
        assert_eq!(result.assignment.values(), &[Some(1)]);
        let BestConstraintTotals::Available(totals) =
            metrics.snapshot().latest.unwrap().best_constraint_totals
        else {
            panic!()
        };
        assert_eq!(totals["positive"].score, SimpleScore::new(1));
        assert_eq!(totals["negative"].score, SimpleScore::new(-1));
    }
}

fn list_candidates(p: &ListProblem<i64>, a: &ListAssignment<i64>) -> Vec<ListMove<i64>> {
    list_assign_moves(p, a)
        .unwrap()
        .chain(list_moves(p, a, Neighborhood::ChangeAndSwap).unwrap())
        .collect()
}
fn mixed_candidates(
    p: &MixedProblem<i64, i64>,
    a: &MixedAssignment<i64, i64>,
) -> Vec<MixedMove<i64, i64>> {
    change_moves(p.basic(), a.basic())
        .unwrap()
        .map(|basic| MixedMove {
            basic,
            lists: ListMove::Composite(vec![]),
        })
        .chain(
            list_candidates(p.lists(), a.lists())
                .into_iter()
                .map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                }),
        )
        .collect()
}

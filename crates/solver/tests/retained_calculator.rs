//! Original stateful calculator-session tests; no upstream method credit.
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
fn normalize<A, S>(mut r: SolverResult<A, S>) -> SolverResult<A, S> {
    r.work.constraint_evaluations = 0;
    for p in &mut r.phases {
        p.work.constraint_evaluations = 0;
    }
    r
}
fn plans() -> Vec<SolverSequence<'static>> {
    let local = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 3,
        acceptance: Acceptance::HillClimbing,
        ..Default::default()
    });
    let phases = vec![
        local.clone(),
        SolverPhase::RuinRecreate(RuinRecreateConfig {
            minimum_ruined: 1,
            maximum_ruined: 1,
            max_attempts: 3,
            seed: 7,
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig::default()),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::Complete,
            ..Default::default()
        }),
    ];
    phases
        .into_iter()
        .flat_map(|phase| {
            [0, 1, 2, 5, 30].map(|cap| SolverSequence {
                limits: SolverLimits {
                    max_evaluations: cap,
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig::default()),
                    local.clone(),
                    phase.clone(),
                ],
            })
        })
        .collect()
}
#[test]
fn all_models_match_factory_plans_and_count_only_actual_calculator_calls() {
    for config in plans() {
        let counts = Rc::new(Counts::default());
        let p = basic();
        let factory = || Cache::<Assignment<i64>>::new(&counts);
        let expected = solve_calculator(&p, &factory, config.clone()).unwrap();
        let mut d =
            CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
        let before = counts.scores.get();
        let factories = counts.factories.get();
        let actual = d.run_solver(config.clone()).unwrap();
        assert_eq!(
            actual.work.constraint_evaluations,
            u128::from(counts.scores.get() - before)
        );
        assert_eq!(counts.factories.get(), factories);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score().unwrap(), score);
        }
        assert_eq!(
            *d.registry().unwrap().total().unwrap(),
            SimpleScore::new(d.assignment().parts().iter().sum())
        );
        assert_eq!(normalize(actual), normalize(expected));

        let counts = Rc::new(Counts::default());
        let p = lists();
        let factory = || Cache::<ListAssignment<i64>>::new(&counts);
        let expected = solve_list_calculator(&p, &factory, config.clone()).unwrap();
        let mut d =
            ListCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
        let before = counts.scores.get();
        let factories = counts.factories.get();
        let actual = d.run_solver(config.clone()).unwrap();
        assert_eq!(
            actual.work.constraint_evaluations,
            u128::from(counts.scores.get() - before)
        );
        assert_eq!(counts.factories.get(), factories);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score().unwrap(), score);
        }
        assert_eq!(
            *d.registry().unwrap().total().unwrap(),
            SimpleScore::new(d.assignment().parts().iter().sum())
        );
        assert_eq!(normalize(actual), normalize(expected));

        let counts = Rc::new(Counts::default());
        let p = MixedProblem::new(basic(), lists());
        let factory = || Cache::<MixedAssignment<i64, i64>>::new(&counts);
        let expected = solve_mixed_calculator(&p, &factory, mixed(config.clone())).unwrap();
        let mut d =
            MixedCalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
        let before = counts.scores.get();
        let factories = counts.factories.get();
        let actual = d.run_solver(mixed(config)).unwrap();
        assert_eq!(
            actual.work.constraint_evaluations,
            u128::from(counts.scores.get() - before)
        );
        assert_eq!(counts.factories.get(), factories);
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score().unwrap(), score);
        }
        assert_eq!(
            *d.registry().unwrap().total().unwrap(),
            SimpleScore::new(d.assignment().parts().iter().sum())
        );
        assert_eq!(normalize(actual), normalize(expected));
    }
}

#[test]
fn custom_trials_and_best_restoration_preserve_cache_and_matches_without_rescoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
    let counts = Rc::new(Counts::default());
    let mut d =
        CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
    let mut events = vec![];
    let mut observer = |event: SolverEvent<'_, Assignment<i64>, SimpleScore>| {
        if let SolverEvent::Best(update) = event {
            events.push(update.score);
        }
    };
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        let value = c.evaluate(
            &Move::Change {
                variable: 0,
                value: Some(1),
            },
            |_, s| s,
        )?;
        assert_eq!(value, Some(SimpleScore::new(1)));
        assert_eq!(c.score(), SimpleScore::ZERO);
        c.execute(&Move::Change {
            variable: 0,
            value: Some(2),
        })?;
        c.execute(&Move::Change {
            variable: 0,
            value: Some(1),
        })?;
        Ok(())
    };
    let r = d
        .run_solver_with_extensions(
            SolverSequence {
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                events: Some(&mut observer),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(d.assignment().values(), &[Some(2), Some(0)]);
    assert_eq!(d.score().unwrap(), SimpleScore::new(2));
    assert_eq!(*d.registry().unwrap().total().unwrap(), SimpleScore::new(2));
    assert_eq!(counts.resets.get(), 1);
    assert_eq!(counts.factories.get(), 1);
    assert_eq!(counts.scores.get(), 4); // One earlier initialization + three trials.
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.work.constraint_evaluations, 3);
    assert!(counts.clones.get() > 0);
    assert_eq!(events, vec![SimpleScore::ZERO, SimpleScore::new(2)]);
    let movement = Move::Change {
        variable: 1,
        value: Some(1),
    };
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(3));
    assert_eq!(*d.registry().unwrap().total().unwrap(), SimpleScore::new(3));
    d.assert_consistent().unwrap();
}

#[test]
fn nonzero_phase_initialization_uses_cached_score_but_still_consumes_the_budget() {
    let counts = Rc::new(Counts::default());
    let p = basic();
    let mut d =
        CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), false)
            .unwrap();
    let before = counts.scores.get();
    let r = d
        .run_solver(SolverSequence {
            limits: SolverLimits {
                max_evaluations: 1,
                ..Default::default()
            },
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
        })
        .unwrap();
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.work.constraint_evaluations, 0);
    assert_eq!(counts.scores.get(), before);
    assert_eq!(r.termination, SolverTermination::EvaluationLimit);
    let token = CancellationToken::default();
    token.cancel();
    let r = d
        .run_solver(SolverSequence {
            limits: SolverLimits {
                cancellation: Some(&token),
                ..Default::default()
            },
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
        })
        .unwrap();
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(counts.scores.get(), before);
}

#[test]
fn diagnostics_use_independent_factories_and_leave_live_matches_usable() {
    let counts = Rc::new(Counts::default());
    let p = basic();
    let mut d =
        CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
    let r = d
        .run_solver(SolverConfig {
            limits: SolverLimits {
                assert_phase_scores: true,
                ..Default::default()
            },
            local_search: vec![
                LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                };
                2
            ],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.phases.len(), 3);
    assert_eq!(counts.factories.get(), 4); // One live session and three independent diagnostics.
    assert!(u128::from(counts.scores.get() - 1) > r.work.constraint_evaluations);
    assert_eq!(d.score().unwrap(), r.score.unwrap());
    d.apply(&Move::Change {
        variable: 0,
        value: Some(0),
    })
    .unwrap()
    .commit();
    assert_eq!(*d.registry().unwrap().total().unwrap(), d.score().unwrap());
}

#[test]
fn representative_phase_reuse_exposes_factory_savings_and_snapshot_cost() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let config = SolverConfig {
        local_search: vec![
            LocalSearchConfig {
                max_steps: 1,
                selection: MoveSelection::FirstAccepted,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            };
            2
        ],
        ..Default::default()
    };
    let fresh = Rc::new(Counts::default());
    let baseline = solve_calculator(
        &p,
        &|| Cache::<Assignment<i64>>::new(&fresh),
        config.clone(),
    )
    .unwrap();
    let counts = Rc::new(Counts::default());
    let mut d = CalculatorScoreDirector::new(
        &p,
        p.initial_assignment(),
        || Cache::<Assignment<i64>>::new(&counts),
        false,
    )
    .unwrap();
    let result = d.run_solver(config).unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(2)));
    assert_eq!(normalize(result), normalize(baseline));
    assert_eq!(
        (
            fresh.factories.get(),
            fresh.resets.get(),
            fresh.scores.get(),
            fresh.clones.get()
        ),
        (3, 3, 6, 0)
    );
    assert_eq!(
        (
            counts.factories.get(),
            counts.resets.get(),
            counts.scores.get(),
            counts.clones.get()
        ),
        (1, 1, 4, 4)
    );
}

#[test]
fn command_failure_retains_last_commit_and_a_later_plan_can_continue() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let counts = Rc::new(Counts::default());
    let mut d =
        CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
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
        c.execute(&Move::Change {
            variable: 0,
            value: Some(1),
        })?;
        Err(CustomPhaseError::Command("stop".into()))
    };
    let result = d.run_solver_with_extensions(
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(SolverError::Custom {
            error: CustomPhaseError::Command(_),
            ..
        })
    ));
    assert_eq!(d.assignment().values(), &[Some(1)]);
    assert_eq!(d.score().unwrap(), SimpleScore::new(1));
    assert_eq!(*d.registry().unwrap().total().unwrap(), SimpleScore::new(1));
    let result = d
        .run_solver(SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            })],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(2)));
    assert_eq!(counts.factories.get(), 1);
}

fn relocate() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 1,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn undo_relocate() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 1,
            index: 0,
        },
        destination: ListPosition {
            entity: 0,
            index: 1,
        },
    }
}
#[test]
fn list_and_mixed_custom_phases_restore_independent_caches_and_match_handles() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let counts = Rc::new(Counts::default());
    let mut d =
        ListCalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        ListAssignment<i64>,
        ListMove<i64>,
        SimpleScore,
        ListScoringError,
    >| {
        c.execute(&relocate())?;
        c.execute(&undo_relocate())?;
        Ok(())
    };
    let r = d
        .run_solver_with_extensions(
            SolverSequence {
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(-3)));
    assert_eq!(d.assignment().lists(), &[vec![1], vec![2]]);
    assert_eq!(
        *d.registry().unwrap().total().unwrap(),
        SimpleScore::new(-3)
    );
    assert_eq!(
        (
            counts.factories.get(),
            counts.resets.get(),
            counts.scores.get()
        ),
        (1, 1, 3)
    );
    d.apply(&undo_relocate()).unwrap().commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-5));
    d.assert_consistent().unwrap();

    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
        p,
    );
    let counts = Rc::new(Counts::default());
    let mut d =
        MixedCalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        MixedAssignment<i64, i64>,
        MixedMove<i64, i64>,
        SimpleScore,
        MixedScoringError,
    >| {
        c.execute(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2),
            },
            lists: relocate(),
        })?;
        c.execute(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(0),
            },
            lists: undo_relocate(),
        })?;
        Ok(())
    };
    let r = d
        .run_solver_with_extensions(
            MixedSolverSequence {
                phases: vec![MixedSolverPhase::Custom(CustomPhaseConfig::default())],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(-1)));
    assert_eq!(
        *d.registry().unwrap().total().unwrap(),
        SimpleScore::new(-1)
    );
    assert_eq!(
        (
            counts.factories.get(),
            counts.resets.get(),
            counts.scores.get()
        ),
        (1, 1, 3)
    );
    d.apply(&MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: undo_relocate(),
    })
    .unwrap()
    .commit();
    assert_eq!(d.score().unwrap(), SimpleScore::new(-4));
    d.assert_consistent().unwrap();
}

struct Provider {
    calls: Rc<Cell<u64>>,
}
impl MoveProvider<FiniteProblem<i64>, Assignment<i64>, Move<i64>> for Provider {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i64>,
        a: &'a Assignment<i64>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i64>> + 'a>
    where
        Move<i64>: 'a,
    {
        self.calls.set(self.calls.get() + 1);
        Box::new((0..a.values().len()).map(|i| Move::Change {
            variable: i,
            value: Some(2),
        }))
    }
}
#[test]
fn providers_and_live_score_limits_use_retained_calculator_phases() {
    let p = basic();
    let counts = Rc::new(Counts::default());
    let calls = Rc::new(Cell::new(0));
    let mut d =
        CalculatorScoreDirector::new(&p, p.initial_assignment(), || Cache::new(&counts), true)
            .unwrap();
    let mut provider = Provider {
        calls: calls.clone(),
    };
    let mut providers = PhaseMoveProviders::default();
    providers.bind_shared([0, 1], &mut provider).unwrap();
    let quota = SearchTermination::Condition(SearchLimit::ScoreEvaluations(5));
    let result = d
        .run_solver_with_extensions(
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&quota),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig::default()),
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 3,
                        ..Default::default()
                    }),
                ],
            },
            SolverExtensions {
                move_providers: providers,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.termination, SolverTermination::Condition);
    assert_eq!(result.work.evaluations, 5);
    assert!(calls.get() > 0);
    assert_eq!(counts.factories.get(), 1);
    assert_eq!(
        result.work.constraint_evaluations,
        u128::from(counts.scores.get() - 1)
    );
}

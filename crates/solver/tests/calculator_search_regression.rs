use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[derive(Default)]
struct Counts {
    resets: Cell<usize>,
    scores: Cell<usize>,
    before: Cell<usize>,
    after: Cell<usize>,
}
#[derive(Default)]
struct Calculator {
    scopes: Vec<Vec<usize>>,
    contributions: Vec<HardSoftScore>,
    handles: Vec<Option<MatchRegistration<HardSoftScore, Vec<usize>>>>,
    total: HardSoftScore,
    counts: Rc<Counts>,
    failure: Rc<Cell<u8>>,
}
fn contribution(scope: &[usize], a: &Assignment<i64>) -> HardSoftScore {
    if scope.len() == 1 {
        HardSoftScore::new(0, -a.values()[scope[0]].unwrap_or(0))
    } else {
        let l = a.values()[scope[0]];
        let r = a.values()[scope[1]];
        HardSoftScore::new(-i64::from(l.is_some() && l == r), 0)
    }
}
fn scopes(n: usize) -> Vec<Vec<usize>> {
    (0..n)
        .map(|i| vec![i])
        .chain((0..n).flat_map(|i| (i + 1..n).map(move |j| vec![i, j])))
        .collect()
}
impl IncrementalCalculator<Assignment<i64>> for Calculator {
    type Score = HardSoftScore;
    type Justification = Vec<usize>;
    type Change = Vec<usize>;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &Assignment<i64>,
        r: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.resets.set(self.counts.resets.get() + 1);
        self.scopes = scopes(a.values().len());
        self.contributions.clear();
        self.handles.clear();
        self.total = HardSoftScore::ZERO;
        for (i, scope) in self.scopes.iter().enumerate() {
            let score = contribution(scope, a);
            self.contributions.push(score);
            self.total = self.total + score;
            self.handles.push(if r.is_enabled() {
                Some(r.register(format!("c{i}"), score, scope.clone())?)
            } else {
                None
            });
        }
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &Assignment<i64>,
        variables: &Vec<usize>,
        r: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.before.set(self.counts.before.get() + 1);
        match self.failure.get() {
            1 => return Err(MatchRegistryError::Calculator("before".into())),
            3 => panic!("before"),
            _ => (),
        }
        assert!(variables.windows(2).all(|v| v[0] < v[1]));
        for (i, scope) in self.scopes.iter().enumerate() {
            if scope.iter().any(|v| variables.contains(v)) {
                self.total = self.total - self.contributions[i];
                if let Some(handle) = self.handles[i].take() {
                    handle.cancel(r)?;
                }
            }
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i64>,
        variables: &Vec<usize>,
        r: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.after.set(self.counts.after.get() + 1);
        for (i, scope) in self.scopes.iter().enumerate() {
            if scope.iter().any(|v| variables.contains(v)) {
                let score = contribution(scope, a);
                self.contributions[i] = score;
                self.total = self.total + score;
                self.handles[i] = if r.is_enabled() {
                    Some(r.register(format!("c{i}"), score, scope.clone())?)
                } else {
                    None
                };
            }
        }
        match self.failure.get() {
            2 => return Err(MatchRegistryError::Calculator("after".into())),
            4 => panic!("after"),
            5 => self.total = self.total + HardSoftScore::new(0, 1),
            _ => (),
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> Self::Score {
        self.counts.scores.set(self.counts.scores.get() + 1);
        if self.failure.get() == 6 {
            panic!("score");
        }
        self.total
    }
}
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2])
            .with_initial(0)
            .with_pinned(true),
        Variable::new(vec![0, 1, 2]),
        Variable::new(vec![0, 1, 2]),
        Variable::new(vec![0, 1, 2]).allowing_unassigned(),
    ])
    .unwrap()
}
fn network(n: usize) -> ConstraintNetwork<'static, i64, HardSoftScore> {
    let mut net = ConstraintNetwork::new(n);
    for (i, scope) in scopes(n).into_iter().enumerate() {
        let pair = scope.len() == 2;
        net.add_constraint(format!("c{i}"), scope, move |v| {
            if pair {
                HardSoftScore::new(-i64::from(v[0].is_some() && v[0] == v[1]), 0)
            } else {
                HardSoftScore::new(0, -v[0].copied().unwrap_or(0))
            }
        })
        .unwrap();
    }
    net
}
fn complete() -> Assignment<i64> {
    Assignment::new(vec![Some(0), Some(0), Some(0), None])
}
fn change(variable: usize, value: Option<i64>) -> Move<i64> {
    Move::Change { variable, value }
}

#[test]
fn move_guards_validate_commit_rollback_and_preserve_explanations() {
    let p = problem();
    let n = network(4);
    let counts = Rc::new(Counts::default());
    let factory = || Calculator {
        counts: counts.clone(),
        ..Default::default()
    };
    let mut d = CalculatorScoreDirector::new(&p, complete(), factory, true).unwrap();
    let initial = d.score().unwrap();
    for movement in [
        change(1, Some(1)),
        Move::Swap { left: 1, right: 2 },
        Move::Composite(vec![
            change(1, Some(1)),
            change(1, Some(2)),
            change(2, Some(1)),
        ]),
        change(2, None),
    ] {
        let candidate = d.apply(&movement).unwrap();
        assert_eq!(
            candidate.score(),
            n.calculate(&p, candidate.assignment()).unwrap()
        );
        drop(candidate);
        assert_eq!(d.assignment(), &complete());
        assert_eq!(d.score().unwrap(), initial);
        assert_eq!(*d.registry().unwrap().total().unwrap(), initial);
        assert_eq!(
            d.registry()
                .unwrap()
                .constraints()
                .unwrap()
                .values()
                .map(|c| c.matches().len())
                .sum::<usize>(),
            10
        );
    }
    let before = counts.before.get();
    assert!(d.apply(&change(0, Some(1))).is_err());
    assert!(d.apply(&change(1, Some(4))).is_err());
    assert!(d.apply(&change(8, Some(1))).is_err());
    assert_eq!(counts.before.get(), before);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = d.apply(&change(1, Some(1))).unwrap();
            panic!("visitor");
        }))
        .is_err()
    );
    assert_eq!(d.assignment(), &complete());
    assert_eq!(d.score().unwrap(), initial);
    d.apply(&change(1, Some(1))).unwrap().commit();
    assert_eq!(d.assignment().values()[1], Some(1));
    assert_eq!(d.score().unwrap(), n.calculate(&p, d.assignment()).unwrap());
    assert_eq!(counts.resets.get(), 1);
    d.assert_consistent().unwrap();
    assert_eq!(counts.resets.get(), 2); // Separate verification calculator only.
    d.reset(p.initial_assignment()).unwrap();
    assert_eq!(d.score().unwrap().structural(), -2);
}

#[test]
fn construction_and_local_search_match_existing_engine_behavior() {
    let p = problem();
    let n = network(4);
    for budget in [0, 1, 2, 3, 7, 100] {
        for interruption in [
            ConstructionInterruption::KeepBestPrefix,
            ConstructionInterruption::DiscardIncompleteStep,
        ] {
            let config = ConstructionConfig {
                max_evaluations: budget,
                interruption,
                optional: OptionalConstruction::EvaluateUnassigned,
                ..Default::default()
            };
            let actual =
                calculator_construction(&p, p.initial_assignment(), Calculator::default, config)
                    .unwrap();
            let mut expected =
                greedy_construction_from(&p, &n, p.initial_assignment(), config).unwrap();
            expected.constraint_evaluations = actual.constraint_evaluations;
            assert_eq!(actual, expected);
            assert_eq!(
                actual.constraint_evaluations,
                u128::from(actual.evaluations)
            );
        }
    }
    for acceptance in [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
    ] {
        for selection in [
            MoveSelection::FirstAccepted,
            MoveSelection::BestAccepted,
            MoveSelection::FirstBestScoreImproving,
        ] {
            for budget in [0, 1, 2, 15, 100] {
                let config = LocalSearchConfig {
                    max_evaluations: budget,
                    max_steps: 12,
                    acceptance,
                    selection,
                    tie_breaking: TieBreaking::Random { seed: 42 },
                    assert_scores: true,
                    ..Default::default()
                };
                let actual =
                    calculator_local_search_from(&p, complete(), Calculator::default, config)
                        .unwrap();
                let mut expected = local_search_from(&p, &n, complete(), config).unwrap();
                expected.constraint_evaluations = actual.constraint_evaluations;
                assert_eq!(actual, expected, "{acceptance:?} {selection:?} {budget}");
                assert_eq!(
                    actual.constraint_evaluations,
                    u128::from(actual.evaluations)
                );
            }
        }
    }
}

#[test]
fn providers_and_custom_commands_use_incremental_callbacks() {
    let p = problem();
    let n = network(4);
    let counts = Rc::new(Counts::default());
    let factory = || Calculator {
        counts: counts.clone(),
        ..Default::default()
    };
    let provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
        vec![change(1, Some(1)), change(2, Some(2)), change(3, Some(2))]
    };
    let config = LocalSearchConfig {
        max_evaluations: 15,
        max_steps: 4,
        selection: MoveSelection::BestAccepted,
        ..Default::default()
    };
    let actual = calculator_local_search_with_provider(
        &p,
        complete(),
        factory,
        config,
        &mut provider.clone(),
    )
    .unwrap();
    let mut expected =
        local_search_from_with_provider(&p, &n, complete(), config, &mut provider.clone()).unwrap();
    expected.constraint_evaluations = actual.constraint_evaluations;
    assert_eq!(actual, expected);
    assert_eq!(counts.resets.get(), 1);
    assert_eq!(counts.scores.get() as u64, actual.evaluations); // No rescoring on winner replay.
    let mut updates = vec![];
    let result = calculator_custom_phase(
        &p,
        complete(),
        Calculator::default,
        CustomPhaseConfig {
            max_evaluations: 10,
            ..Default::default()
        },
        |c| {
            let baseline = c.score();
            let candidate = c
                .evaluate(&change(1, Some(1)), |a, score| (a.clone(), score))?
                .unwrap();
            assert_eq!(candidate.1, n.calculate(&p, &candidate.0).unwrap());
            assert_eq!(c.score(), baseline);
            assert_eq!(c.assignment(), &complete());
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = c.evaluate(&change(2, Some(2)), |_, _| panic!("visitor"));
                }))
                .is_err()
            );
            assert_eq!(c.assignment(), &complete());
            c.execute(&change(1, Some(1)))?;
            c.execute(&change(2, Some(2)))?;
            c.execute(&change(2, Some(0)))?;
            c.assert_consistent()?;
            Ok(())
        },
        |update| updates.push(update.score),
    )
    .unwrap();
    assert_eq!(
        result.assignment.values(),
        &[Some(0), Some(1), Some(2), None]
    );
    assert_eq!(result.score, Some(HardSoftScore::new(0, -3)));
    assert_eq!(result.work.evaluations, 6);
    assert_eq!(updates.len(), 3);
}

#[test]
fn callback_errors_panics_and_corruption_are_visible() {
    let p = problem();
    let failure = Rc::new(Cell::new(0));
    let factory = || Calculator {
        failure: failure.clone(),
        ..Default::default()
    };
    for mode in 1..=6 {
        failure.set(0);
        let mut d = CalculatorScoreDirector::new(&p, complete(), factory, true).unwrap();
        failure.set(mode);
        if [3, 4, 6].contains(&mode) {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = d.apply(&change(1, Some(1)));
                }))
                .is_err()
            );
            assert!(d.score().is_err());
        } else if mode == 5 {
            d.apply(&change(1, Some(1))).unwrap().commit();
            assert!(matches!(
                d.assert_consistent(),
                Err(ScoringError::ScoreCorruption(_))
            ));
        } else {
            assert!(d.apply(&change(1, Some(1))).is_err());
            assert!(d.score().is_err());
        }
        if mode != 5 {
            assert_eq!(d.assignment(), &complete());
        }
        failure.set(0);
        d.reset(complete()).unwrap();
        d.assert_consistent().unwrap();
        assert!(d.score().is_ok());
    }
    // Inverse callbacks may fail too; the guard must restore data and poison state.
    for mode in [1, 2, 3, 4] {
        failure.set(0);
        let mut d = CalculatorScoreDirector::new(&p, complete(), factory, false).unwrap();
        let trial = d.apply(&change(1, Some(1))).unwrap();
        failure.set(mode);
        drop(trial);
        assert_eq!(d.assignment(), &complete());
        assert!(d.score().is_err());
        failure.set(0);
        d.reset(complete()).unwrap();
        d.assert_consistent().unwrap();
    }
}

#[test]
fn zero_budgets_and_invalid_models_never_start_calculators() {
    let p = problem();
    let counts = Rc::new(Counts::default());
    let factory = || Calculator {
        counts: counts.clone(),
        ..Default::default()
    };
    let config = LocalSearchConfig {
        max_evaluations: 0,
        ..Default::default()
    };
    assert_eq!(
        calculator_local_search_from(&p, complete(), factory, config)
            .unwrap()
            .evaluations,
        0
    );
    assert!(calculator_local_search_from(&p, p.initial_assignment(), factory, config).is_err());
    assert!(
        calculator_construction(
            &p,
            Assignment::new(vec![]),
            factory,
            ConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    let c = calculator_custom_phase(
        &p,
        complete(),
        factory,
        CustomPhaseConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        |_| panic!("command"),
        |_| panic!("observer"),
    )
    .unwrap();
    assert_eq!(c.work.evaluations, 0);
    assert_eq!(counts.resets.get(), 0);
}

#[test]
fn search_and_custom_phase_propagate_calculator_failures() {
    let p = problem();
    let failure = Rc::new(Cell::new(1));
    let factory = || Calculator {
        failure: failure.clone(),
        ..Default::default()
    };
    let result =
        calculator_local_search_from(&p, complete(), factory, LocalSearchConfig::default());
    assert!(matches!(
        result,
        Err(LocalSearchError::Scoring(ScoringError::Calculator(_)))
    ));
    let result = calculator_custom_phase(
        &p,
        complete(),
        factory,
        CustomPhaseConfig::default(),
        |c| {
            assert!(c.execute(&change(1, Some(1))).is_err());
            Ok(())
        },
        |_| {},
    );
    assert!(matches!(
        result,
        Err(CustomPhaseError::IgnoredOperationError)
    ));
}

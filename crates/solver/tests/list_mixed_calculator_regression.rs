use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Default)]
struct Cache {
    scores: Vec<i64>,
    handles: Vec<Option<MatchRegistration<SimpleScore, usize>>>,
    total: i64,
    resets: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}
fn list_score(list: &[i64]) -> i64 {
    -list
        .iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * v)
        .sum::<i64>()
}
impl Cache {
    fn reset(
        &mut self,
        scores: Vec<i64>,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.resets.set(self.resets.get() + 1);
        self.total = scores.iter().sum();
        self.scores = scores;
        self.handles = self
            .scores
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if r.is_enabled() {
                    r.register(format!("owner{i}"), SimpleScore::new(*s), i)
                        .map(Some)
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(())
    }
    fn before(
        &mut self,
        changed: &[usize],
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        assert!(changed.windows(2).all(|v| v[0] < v[1]));
        for &i in changed {
            self.total -= self.scores[i];
            if let Some(h) = self.handles[i].take() {
                h.cancel(r)?;
            }
        }
        Ok(())
    }
    fn after(
        &mut self,
        changed: &[usize],
        score: impl Fn(usize) -> i64,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        for &i in changed {
            self.scores[i] = score(i);
            self.total += self.scores[i];
            if r.is_enabled() {
                self.handles[i] =
                    Some(r.register(format!("owner{i}"), SimpleScore::new(self.scores[i]), i)?);
            }
        }
        if self.fail.get() {
            return Err(MatchRegistryError::Calculator("after".into()));
        }
        Ok(())
    }
}
#[derive(Default)]
struct Lists(Cache);
impl IncrementalCalculator<ListAssignment<i64>> for Lists {
    type Score = SimpleScore;
    type Justification = usize;
    type Change = Vec<usize>;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &ListAssignment<i64>,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0
            .reset(a.lists().iter().map(|v| list_score(v)).collect(), r)
    }
    fn before_change(
        &mut self,
        _: &ListAssignment<i64>,
        c: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0.before(c, r)
    }
    fn after_change(
        &mut self,
        a: &ListAssignment<i64>,
        c: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0.after(c, |i| list_score(&a.lists()[i]), r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0.total)
    }
}
fn mixed_score(a: &MixedAssignment<i64, i64>, i: usize) -> i64 {
    list_score(&a.lists().lists()[i]) * a.basic().values()[i].unwrap_or(1)
}
fn affected(c: &MixedCalculatorChange) -> Vec<usize> {
    let mut v = c.basic_variables.clone();
    v.extend(&c.list_entities);
    v.sort_unstable();
    v.dedup();
    v
}
#[derive(Default)]
struct Mixed(Cache);
impl IncrementalCalculator<MixedAssignment<i64, i64>> for Mixed {
    type Score = SimpleScore;
    type Justification = usize;
    type Change = MixedCalculatorChange;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &MixedAssignment<i64, i64>,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0.reset(
            (0..a.lists().lists().len())
                .map(|i| mixed_score(a, i))
                .collect(),
            r,
        )
    }
    fn before_change(
        &mut self,
        _: &MixedAssignment<i64, i64>,
        c: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0.before(&affected(c), r)
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<i64, i64>,
        c: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.0.after(&affected(c), |i| mixed_score(a, i), r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0.total)
    }
}
fn list_problem() -> ListProblem<i64> {
    ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1]).with_pinned_prefix(1),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
}
fn list_network() -> ListConstraintNetwork<'static, i64, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, 4);
    for i in 0..2 {
        n.add_constraint(format!("owner{i}"), vec![ListScope::Entity(i)], |f| {
            let ListFact::Entity(v) = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(list_score(v))
        })
        .unwrap();
    }
    n
}
fn mixed_problem() -> MixedProblem<i64, i64> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]), Variable::new(vec![1, 2])]).unwrap(),
        list_problem(),
    )
}
fn mixed_network() -> MixedConstraintNetwork<'static, i64, i64, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(2, 2, 4);
    for i in 0..2 {
        n.add_constraint(
            format!("owner{i}"),
            vec![MixedScope::Basic(i), MixedScope::ListEntity(i)],
            |f| {
                let (MixedFact::Basic(b), MixedFact::ListEntity(v)) = (&f[0], &f[1]) else {
                    unreachable!()
                };
                SimpleScore::new(list_score(v) * b.copied().unwrap_or(1))
            },
        )
        .unwrap();
    }
    n
}
fn complete_list() -> ListAssignment<i64> {
    ListAssignment::new(vec![vec![1, 4, 3], vec![2]])
}
fn complete_mixed() -> MixedAssignment<i64, i64> {
    MixedAssignment::new(Assignment::new(vec![Some(2), Some(2)]), complete_list())
}
fn swap() -> ListMove<i64> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 1,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn joint() -> MixedMove<i64, i64> {
    MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: swap(),
    }
}

#[test]
fn list_and_joint_transactions_preserve_scores_and_matches() {
    let p = list_problem();
    let n = list_network();
    let resets = Rc::new(Cell::new(0));
    let factory = || {
        Lists(Cache {
            resets: resets.clone(),
            ..Default::default()
        })
    };
    let mut d = ListCalculatorScoreDirector::new(&p, complete_list(), factory, true).unwrap();
    for movement in [
        swap(),
        ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 3,
            destination: ListPosition {
                entity: 1,
                index: 1,
            },
            reverse: true,
        },
        ListMove::Unassign {
            entity: 0,
            start: 1,
            end: 3,
        },
        ListMove::Composite(vec![
            ListMove::Unassign {
                entity: 0,
                start: 1,
                end: 2,
            },
            ListMove::Assign {
                value: 4,
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            },
        ]),
    ] {
        let c = d.apply(&movement).unwrap();
        assert_eq!(c.score(), n.calculate(&p, c.assignment()).unwrap());
        drop(c);
        assert_eq!(d.assignment(), &complete_list());
        assert_eq!(*d.registry().unwrap().total().unwrap(), d.score().unwrap());
    }
    assert!(
        d.apply(&ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1
        })
        .is_err()
    );
    assert_eq!(resets.get(), 1);
    let p = mixed_problem();
    let n = mixed_network();
    let mut d =
        MixedCalculatorScoreDirector::new(&p, complete_mixed(), Mixed::default, true).unwrap();
    let c = d.apply(&joint()).unwrap();
    assert_eq!(c.score(), n.calculate(&p, c.assignment()).unwrap());
    drop(c);
    assert_eq!(d.assignment(), &complete_mixed());
    d.assert_consistent().unwrap();
    let invalid = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
    };
    assert!(d.apply(&invalid).is_err());
    assert_eq!(d.assignment(), &complete_mixed());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _c = d.apply(&joint()).unwrap();
            panic!("visitor");
        }))
        .is_err()
    );
    d.assert_consistent().unwrap();
    assert_eq!(d.assignment(), &complete_mixed());
    d.apply(&joint()).unwrap().commit();
    assert_eq!(d.score().unwrap(), n.calculate(&p, d.assignment()).unwrap());
    assert_eq!(*d.registry().unwrap().total().unwrap(), d.score().unwrap());
}
#[test]
fn construction_and_search_agree_with_scoped_backends() {
    let lp = list_problem();
    let ln = list_network();
    let mp = mixed_problem();
    let mn = mixed_network();
    for budget in [0, 1, 2, 5, 20, 100] {
        for interruption in [
            ConstructionInterruption::KeepBestPrefix,
            ConstructionInterruption::DiscardIncompleteStep,
        ] {
            let config = ConstructionConfig {
                max_evaluations: budget,
                interruption,
                ..Default::default()
            };
            let actual =
                list_calculator_construction(&lp, lp.initial_assignment(), Lists::default, config)
                    .unwrap();
            let mut expected =
                greedy_list_construction_from(&lp, &ln, lp.initial_assignment(), config).unwrap();
            expected.constraint_evaluations = actual.constraint_evaluations;
            assert_eq!(actual, expected);
            for order in [
                MixedConstructionOrder::BasicThenLists,
                MixedConstructionOrder::ListsThenBasic,
            ] {
                let settings = MixedConstructionConfig {
                    construction: config,
                    order,
                };
                let actual = mixed_calculator_construction(
                    &mp,
                    mp.initial_assignment(),
                    Mixed::default,
                    settings,
                )
                .unwrap();
                let mut expected =
                    greedy_mixed_construction_from(&mp, &mn, mp.initial_assignment(), settings)
                        .unwrap();
                expected.constraint_evaluations = actual.constraint_evaluations;
                assert_eq!(actual, expected);
            }
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
        for budget in [0, 1, 3, 30] {
            let config = LocalSearchConfig {
                max_evaluations: budget,
                max_steps: 5,
                acceptance,
                selection: MoveSelection::BestAccepted,
                tie_breaking: TieBreaking::Random { seed: 31 },
                assert_scores: true,
                ..Default::default()
            };
            let actual = list_calculator_local_search_from(
                &lp,
                complete_list(),
                Lists::default,
                config.clone(),
            )
            .unwrap();
            let mut expected =
                list_local_search_from(&lp, &ln, complete_list(), config.clone()).unwrap();
            expected.constraint_evaluations = actual.constraint_evaluations;
            assert_eq!(actual, expected);
            for joint_moves in [false, true] {
                let settings = MixedLocalSearchConfig {
                    search: config.clone(),
                    joint_moves,
                };
                let actual = mixed_calculator_local_search_from(
                    &mp,
                    complete_mixed(),
                    Mixed::default,
                    settings.clone(),
                )
                .unwrap();
                let mut expected =
                    mixed_local_search_from(&mp, &mn, complete_mixed(), settings).unwrap();
                expected.constraint_evaluations = actual.constraint_evaluations;
                assert_eq!(actual, expected);
            }
        }
    }
}
#[test]
fn lazy_providers_and_custom_phases_use_both_backends() {
    let lp = list_problem();
    let ln = list_network();
    let mp = mixed_problem();
    let mn = mixed_network();
    let cfg = LocalSearchConfig {
        max_evaluations: 12,
        max_steps: 4,
        ..Default::default()
    };
    let provider = |_: &ListProblem<i64>, _: &ListAssignment<i64>, _: u64| vec![swap()];
    let actual = list_calculator_local_search_with_provider(
        &lp,
        complete_list(),
        Lists::default,
        cfg.clone(),
        &mut provider.clone(),
    )
    .unwrap();
    let mut expected = list_local_search_from_with_provider(
        &lp,
        &ln,
        complete_list(),
        cfg.clone(),
        &mut provider.clone(),
    )
    .unwrap();
    expected.constraint_evaluations = actual.constraint_evaluations;
    assert_eq!(actual, expected);
    let provider =
        |_: &MixedProblem<i64, i64>, _: &MixedAssignment<i64, i64>, _: u64| vec![joint()];
    let settings = MixedLocalSearchConfig {
        search: cfg,
        joint_moves: true,
    };
    let actual = mixed_calculator_local_search_with_provider(
        &mp,
        complete_mixed(),
        Mixed::default,
        settings.clone(),
        &mut provider.clone(),
    )
    .unwrap();
    let mut expected = mixed_local_search_from_with_provider(
        &mp,
        &mn,
        complete_mixed(),
        settings,
        &mut provider.clone(),
    )
    .unwrap();
    expected.constraint_evaluations = actual.constraint_evaluations;
    assert_eq!(actual, expected);
    let mut seen = 0;
    let list = list_calculator_custom_phase(
        &lp,
        complete_list(),
        Lists::default,
        CustomPhaseConfig::default(),
        |c| {
            c.evaluate(&swap(), |a, s| assert_eq!(s, ln.calculate(&lp, a).unwrap()))?;
            assert_eq!(c.assignment(), &complete_list());
            c.execute(&swap())?;
            c.assert_consistent()?;
            Ok(())
        },
        |_| seen += 1,
    )
    .unwrap();
    assert!(seen > 0);
    assert_eq!(list.work.evaluations, 3);
    let mixed = mixed_calculator_custom_phase(
        &mp,
        complete_mixed(),
        Mixed::default,
        CustomPhaseConfig::default(),
        |c| {
            c.evaluate(&joint(), |a, s| {
                assert_eq!(s, mn.calculate(&mp, a).unwrap())
            })?;
            assert_eq!(c.assignment(), &complete_mixed());
            c.execute(&joint())?;
            c.assert_consistent()?;
            Ok(())
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(mixed.work.evaluations, 3);
}
#[test]
fn failed_inverse_updates_restore_assignments_and_require_reset() {
    let lp = list_problem();
    let mp = mixed_problem();
    let fail = Rc::new(Cell::new(false));
    let mut d = ListCalculatorScoreDirector::new(
        &lp,
        complete_list(),
        || {
            Lists(Cache {
                fail: fail.clone(),
                ..Default::default()
            })
        },
        true,
    )
    .unwrap();
    let trial = d.apply(&swap()).unwrap();
    fail.set(true);
    drop(trial);
    assert_eq!(d.assignment(), &complete_list());
    assert!(d.score().is_err());
    fail.set(false);
    d.reset(complete_list()).unwrap();
    d.assert_consistent().unwrap();
    let mut d = MixedCalculatorScoreDirector::new(
        &mp,
        complete_mixed(),
        || {
            Mixed(Cache {
                fail: fail.clone(),
                ..Default::default()
            })
        },
        true,
    )
    .unwrap();
    let trial = d.apply(&joint()).unwrap();
    fail.set(true);
    drop(trial);
    assert_eq!(d.assignment(), &complete_mixed());
    assert!(d.score().is_err());
    fail.set(false);
    d.reset(complete_mixed()).unwrap();
    d.assert_consistent().unwrap();
}

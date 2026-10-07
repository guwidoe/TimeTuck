use std::{cell::Cell, fmt::Debug};
use timetuck::*;
#[derive(Default)]
struct Basic(i64);
impl IncrementalCalculator<Assignment<i64>> for Basic {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = a.values().iter().flatten().sum();
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &Assignment<i64>,
        c: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for &i in c {
            self.0 -= a.values()[i].unwrap_or(0);
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i64>,
        c: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for &i in c {
            self.0 += a.values()[i].unwrap_or(0);
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
fn cost(list: &[i64]) -> i64 {
    -list
        .iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * v)
        .sum::<i64>()
}
#[derive(Default)]
struct Lists {
    parts: Vec<i64>,
    sum: i64,
}
impl IncrementalCalculator<ListAssignment<i64>> for Lists {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &ListAssignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.parts = a.lists().iter().map(|l| cost(l)).collect();
        self.sum = self.parts.iter().sum();
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &ListAssignment<i64>,
        c: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for &i in c {
            self.sum -= self.parts[i];
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &ListAssignment<i64>,
        c: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        for &i in c {
            self.parts[i] = cost(&a.lists()[i]);
            self.sum += self.parts[i];
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.sum)
    }
}
#[derive(Default)]
struct Mixed {
    basic: Basic,
    lists: Lists,
}
impl IncrementalCalculator<MixedAssignment<i64, i64>> for Mixed {
    type Score = SimpleScore;
    type Justification = ();
    type Change = MixedCalculatorChange;
    fn reset(
        &mut self,
        a: &MixedAssignment<i64, i64>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.basic.reset(a.basic(), r)?;
        self.lists.reset(a.lists(), r)
    }
    fn before_change(
        &mut self,
        a: &MixedAssignment<i64, i64>,
        c: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.basic.before_change(a.basic(), &c.basic_variables, r)?;
        self.lists.before_change(a.lists(), &c.list_entities, r)
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<i64, i64>,
        c: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.basic.after_change(a.basic(), &c.basic_variables, r)?;
        self.lists.after_change(a.lists(), &c.list_entities, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.basic.calculate_score() + self.lists.calculate_score()
    }
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![]); 2]).unwrap()
}
fn mixed() -> MixedProblem<i64, i64> {
    MixedProblem::new(basic(), lists())
}
fn bn() -> ConstraintNetwork<'static, i64, SimpleScore> {
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(format!("b{i}"), vec![i], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
    }
    n
}
fn ln() -> ListConstraintNetwork<'static, i64, SimpleScore> {
    let mut n = ListConstraintNetwork::new(2, 2);
    for i in 0..2 {
        n.add_constraint(format!("l{i}"), vec![ListScope::Entity(i)], |f| {
            let ListFact::Entity(v) = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(cost(v))
        })
        .unwrap();
    }
    n
}
fn mn() -> MixedConstraintNetwork<'static, i64, i64, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    for i in 0..2 {
        n.add_constraint(format!("b{i}"), vec![MixedScope::Basic(i)], |f| {
            let MixedFact::Basic(v) = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(v.copied().unwrap_or(0))
        })
        .unwrap();
        n.add_constraint(format!("l{i}"), vec![MixedScope::ListEntity(i)], |f| {
            let MixedFact::ListEntity(v) = &f[0] else {
                unreachable!()
            };
            SimpleScore::new(cost(v))
        })
        .unwrap();
    }
    n
}
fn same<A: Eq + Debug>(mut a: SolverResult<A, SimpleScore>, mut b: SolverResult<A, SimpleScore>) {
    a.work.constraint_evaluations = 0;
    b.work.constraint_evaluations = 0;
    for p in &mut a.phases {
        p.work.constraint_evaluations = 0;
    }
    for p in &mut b.phases {
        p.work.constraint_evaluations = 0;
    }
    assert_eq!(a, b);
}
fn config(budget: u64, strategy: ExactSearchStrategy) -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            max_evaluations: budget,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: 12,
                max_steps: 3,
                tie_breaking: TieBreaking::Random { seed: 5 },
                assert_scores: true,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 1,
                maximum_ruined: 1,
                max_attempts: 3,
                seed: 29,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig {
                strategy,
                ..Default::default()
            }),
        ],
    }
}
fn mixed_config(c: SolverSequence<'static>) -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Construction(construction) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction,
                        order: MixedConstructionOrder::ListsThenBasic,
                    })
                }
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search,
                        order: MixedConstructionOrder::BasicThenLists,
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    }
}
#[test]
fn all_phase_types_and_budgets_match_scoped_solvers() {
    let b = basic();
    let l = lists();
    let m = mixed();
    let bn = bn();
    let ln = ln();
    let mn = mn();
    let strategies = [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
    ]
    .into_iter()
    .chain(
        [
            NodeExploration::DepthFirst,
            NodeExploration::BreadthFirst,
            NodeExploration::ScoreFirst,
            NodeExploration::OptimisticBoundFirst,
        ]
        .map(|exploration| {
            ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                exploration,
                ..Default::default()
            })
        }),
    );
    for strategy in strategies {
        for budget in [0, 1, 3, 12, 1000] {
            let c = config(budget, strategy.clone());
            let mut seen = vec![];
            let mut expected = vec![];
            let actual = solve_calculator_with_events(&b, &Basic::default, c.clone(), |e| {
                seen.push(format!("{e:?}"))
            })
            .unwrap();
            let reference =
                solve_with_events(&b, &bn, c.clone(), |e| expected.push(format!("{e:?}"))).unwrap();
            same(actual, reference);
            assert_eq!(seen, expected);
            seen.clear();
            expected.clear();
            let actual = solve_list_calculator_with_events(&l, &Lists::default, c.clone(), |e| {
                seen.push(format!("{e:?}"))
            })
            .unwrap();
            let reference =
                solve_list_with_events(&l, &ln, c.clone(), |e| expected.push(format!("{e:?}")))
                    .unwrap();
            same(actual, reference);
            assert_eq!(seen, expected);
            seen.clear();
            expected.clear();
            let c = mixed_config(c);
            let actual = solve_mixed_calculator_with_events(&m, &Mixed::default, c.clone(), |e| {
                seen.push(format!("{e:?}"))
            })
            .unwrap();
            let reference =
                solve_mixed_with_events(&m, &mn, c, |e| expected.push(format!("{e:?}"))).unwrap();
            same(actual, reference);
            assert_eq!(seen, expected);
        }
    }
}
#[test]
fn shared_limits_and_best_observers_match() {
    let b = basic();
    let l = lists();
    let m = mixed();
    let bn = bn();
    let ln = ln();
    let mn = mn();
    let condition = SearchTermination::Condition(SearchLimit::Steps(1));
    for mode in 0..5 {
        let mut c = config(1000, ExactSearchStrategy::Enumerate);
        match mode {
            0 => c.limits.max_move_evaluations = Some(3),
            1 => c.limits.max_unimproved_steps = Some(1),
            2 => c.limits.best_score_limit = Some(SimpleScore::new(0)),
            3 => c.limits.stop_when_feasible = true,
            _ => c.limits.termination = Some(&condition),
        }
        let mut seen = vec![];
        let mut expected = vec![];
        let a = solve_calculator_with_observer(&b, &Basic::default, c.clone(), |u| {
            seen.push(format!("{u:?}"))
        })
        .unwrap();
        let e =
            solve_with_observer(&b, &bn, c.clone(), |u| expected.push(format!("{u:?}"))).unwrap();
        same(a, e);
        assert_eq!(seen, expected);
        seen.clear();
        expected.clear();
        let a = solve_list_calculator_with_observer(&l, &Lists::default, c.clone(), |u| {
            seen.push(format!("{u:?}"))
        })
        .unwrap();
        let e = solve_list_with_observer(&l, &ln, c.clone(), |u| expected.push(format!("{u:?}")))
            .unwrap();
        same(a, e);
        assert_eq!(seen, expected);
        // Shorten the borrowed condition's lifetime through a direct mixed plan.
        let mc = MixedSolverSequence {
            limits: c.limits,
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig::default()),
            ],
        };
        seen.clear();
        expected.clear();
        let a = solve_mixed_calculator_with_observer(&m, &Mixed::default, mc.clone(), |u| {
            seen.push(format!("{u:?}"))
        })
        .unwrap();
        let e =
            solve_mixed_with_observer(&m, &mn, mc, |u| expected.push(format!("{u:?}"))).unwrap();
        same(a, e);
        assert_eq!(seen, expected);
    }
}
#[test]
fn frozen_zero_cancelled_and_unbound_plans_skip_factories() {
    let counts = Cell::new(0);
    let factory = || {
        counts.set(counts.get() + 1);
        Basic::default()
    };
    let p = basic();
    let zero = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        solve_calculator(&p, &factory, zero)
            .unwrap()
            .work
            .evaluations,
        0
    );
    let token = CancellationToken::new();
    token.cancel();
    let c = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(&token),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        solve_calculator(&p, &factory, c).unwrap().termination,
        SolverTermination::Cancelled
    );
    let plan = SolverSequence {
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        ..Default::default()
    };
    assert!(matches!(
        solve_calculator(&p, &factory, plan),
        Err(SolverError::UnboundCustomPhase { phase: 0 })
    ));
    assert_eq!(counts.get(), 0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1).with_pinned(true);
        2
    ])
    .unwrap();
    let r = solve_calculator(&p, &factory, SolverConfig::default()).unwrap();
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(counts.get(), 1);
    let p = ListProblem::new(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![2]).with_pinned(true),
        ],
    )
    .unwrap();
    same(
        solve_list_calculator(&p, &Lists::default, SolverConfig::default()).unwrap(),
        solve_list(&p, &ln(), SolverConfig::default()).unwrap(),
    );
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![1]).with_initial(1).with_pinned(true);
            2
        ])
        .unwrap(),
        p,
    );
    same(
        solve_mixed_calculator(&p, &Mixed::default, MixedSolverConfig::default()).unwrap(),
        solve_mixed(&p, &mn(), MixedSolverConfig::default()).unwrap(),
    );
}

fn extensions_plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 2,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            }),
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    }
}
macro_rules! extensions_case {
    ($name:ident,$P:ty,$A:ty,$M:ty,$E:ty,$problem:ident,$network:ident,$calculator:ty,$solve:ident,$reference:ident,$plan:expr,$initial:expr,$local:expr,$custom:expr) => {
        #[test]
        fn $name() {
            let run = |calculator: bool| {
                let p = $problem();
                let n = $network();
                let mut events = vec![];
                let mut placements = 0;
                let mut searches = 0;
                let mut commands = 0;
                let mut construct = |_: &$P, _: &$A, _: u64| {
                    placements += 1;
                    vec![$initial]
                };
                let mut local = |_: &$P, _: &$A, _: u64| {
                    searches += 1;
                    vec![$local]
                };
                let mut command = |phase: usize,
                                   c: &mut CustomPhaseContext<'_, $A, $M, SimpleScore, $E>|
                 -> Result<(), CustomPhaseError<$E>> {
                    assert_eq!(phase, 2);
                    commands += 1;
                    let before = c.assignment().clone();
                    let score = c.score();
                    c.evaluate(&$custom, |_, _| ())?;
                    assert_eq!(c.assignment(), &before);
                    assert_eq!(c.score(), score);
                    c.execute(&$custom)?;
                    c.assert_consistent()?;
                    Ok(())
                };
                let mut observer =
                    |e: SolverEvent<'_, $A, SimpleScore>| events.push(format!("{e:?}"));
                let mut extensions = SolverExtensions {
                    command: Some(&mut command),
                    events: Some(&mut observer),
                    ..Default::default()
                };
                extensions.move_providers.bind(0, &mut construct);
                extensions.move_providers.bind(1, &mut local);
                let r = if calculator {
                    $solve(&p, &<$calculator>::default, $plan, extensions).unwrap()
                } else {
                    $reference(&p, &n, $plan, extensions).unwrap()
                };
                assert!(placements > 0);
                assert!(searches > 0);
                assert_eq!(commands, 1);
                assert!(r.is_optimal());
                (r, events, placements, searches)
            };
            let (a, events, placements, searches) = run(true);
            let (b, expected, p, s) = run(false);
            same(a, b);
            assert_eq!(events, expected);
            assert_eq!((placements, searches), (p, s));
        }
    };
}
fn initialize_basic() -> Move<i64> {
    Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(0),
        },
        Move::Change {
            variable: 1,
            value: Some(0),
        },
    ])
}
fn initialize_lists() -> ListMove<i64> {
    ListMove::Composite(vec![
        ListMove::Assign {
            value: 1,
            destination: ListPosition {
                entity: 0,
                index: 0,
            },
        },
        ListMove::Assign {
            value: 2,
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    ])
}
fn swap_lists() -> ListMove<i64> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn move_list() -> ListMove<i64> {
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
extensions_case!(
    basic_extensions,
    FiniteProblem<i64>,
    Assignment<i64>,
    Move<i64>,
    ScoringError,
    basic,
    bn,
    Basic,
    solve_calculator_with_extensions,
    solve_with_extensions,
    extensions_plan(),
    initialize_basic(),
    Move::Change {
        variable: 1,
        value: Some(2)
    },
    Move::Change {
        variable: 0,
        value: Some(2)
    }
);
extensions_case!(
    list_extensions,
    ListProblem<i64>,
    ListAssignment<i64>,
    ListMove<i64>,
    ListScoringError,
    lists,
    ln,
    Lists,
    solve_list_calculator_with_extensions,
    solve_list_with_extensions,
    extensions_plan(),
    initialize_lists(),
    swap_lists(),
    move_list()
);
extensions_case!(mixed_extensions,MixedProblem<i64,i64>,MixedAssignment<i64,i64>,MixedMove<i64,i64>,MixedScoringError,mixed,mn,Mixed,solve_mixed_calculator_with_extensions,solve_mixed_with_extensions,mixed_config(extensions_plan()),MixedMove{basic:initialize_basic(),lists:initialize_lists()},MixedMove{basic:Move::Change{variable:1,value:Some(2)},lists:swap_lists()},MixedMove{basic:Move::Change{variable:0,value:Some(2)},lists:move_list()});

#[test]
fn validation_and_observer_cancellation_precede_future_factories() {
    let p = basic();
    let count = Cell::new(0);
    let factory = || {
        count.set(count.get() + 1);
        Basic::default()
    };
    let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
        panic!("invalid provider must not run")
    };
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(9, &mut provider);
    assert!(matches!(
        solve_calculator_with_extensions(&p, &factory, SolverConfig::default(), extensions),
        Err(SolverError::InvalidMoveProviderPhase { phase: 9 })
    ));
    assert_eq!(count.get(), 0);
    let token = CancellationToken::new();
    let plan = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(&token),
            ..Default::default()
        },
        ..Default::default()
    };
    let r = solve_calculator_with_events(&p, &factory, plan, |e| {
        if matches!(e, SolverEvent::FirstInitialized { .. }) {
            token.cancel();
        }
    })
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(count.get(), 1);
    assert!(r.complete);
}

#[test]
fn completion_retains_decisions_across_all_nine_scoring_routes() {
    let b = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(1),
        Variable::new(vec![0, 1, 2]),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    for unrestricted_after in [false, true] {
        let mut c = SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::Complete,
                ..Default::default()
            })],
            ..Default::default()
        };
        if unrestricted_after {
            c.phases
                .push(SolverPhase::Exact(ExactPhaseConfig::default()));
        }
        let expected = solve(&b, &bn(), c.clone()).unwrap();
        assert_eq!(
            expected.score,
            Some(SimpleScore::new(if unrestricted_after { 4 } else { 3 }))
        );
        assert_eq!(expected.is_optimal(), unrestricted_after);
        assert_eq!(
            expected.phases[0].termination,
            PhaseTermination::Exact(ExactPhaseTermination::CompletionExhausted)
        );
        same(
            solve_calculator(&b, &Basic::default, c.clone()).unwrap(),
            expected.clone(),
        );
        same(
            solve_stream(
                &b,
                || {
                    let mut g = StreamGraphBuilder::new();
                    let s = g.source::<usize, (usize, Option<i64>)>();
                    g.score("basic", s.port(), |(_, v)| SimpleScore::new(v.unwrap_or(0)))?;
                    Ok((g.build(), s))
                },
                c.clone(),
            )
            .unwrap(),
            expected,
        );
        let expected = solve_list(&l, &ln(), c.clone()).unwrap();
        assert_eq!(
            expected.score,
            Some(SimpleScore::new(if unrestricted_after { -3 } else { -5 }))
        );
        assert_eq!(expected.is_optimal(), unrestricted_after);
        same(
            solve_list_calculator(&l, &Lists::default, c.clone()).unwrap(),
            expected.clone(),
        );
        same(
            solve_list_stream(
                &l,
                || {
                    let mut g = StreamGraphBuilder::new();
                    let s = ListStreamSources::new(&mut g);
                    g.score("list", s.entities.port(), |(_, l)| {
                        SimpleScore::new(cost(l))
                    })?;
                    Ok((g.build(), s))
                },
                c.clone(),
            )
            .unwrap(),
            expected,
        );
        let c = mixed_config(c);
        let expected = solve_mixed(&m, &mn(), c.clone()).unwrap();
        assert_eq!(
            expected.score,
            Some(SimpleScore::new(if unrestricted_after { 1 } else { -2 }))
        );
        assert_eq!(expected.is_optimal(), unrestricted_after);
        same(
            solve_mixed_calculator(&m, &Mixed::default, c.clone()).unwrap(),
            expected.clone(),
        );
        same(
            solve_mixed_stream(
                &m,
                || {
                    let mut g = StreamGraphBuilder::new();
                    let s = MixedStreamSources::new(&mut g);
                    g.score("basic", s.basic.port(), |(_, v)| {
                        SimpleScore::new(v.unwrap_or(0))
                    })?;
                    g.score("list", s.lists.entities.port(), |(_, l)| {
                        SimpleScore::new(cost(l))
                    })?;
                    Ok((g.build(), s))
                },
                c,
            )
            .unwrap(),
            expected,
        );
    }
}

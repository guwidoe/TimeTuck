//! Original preparation and actual search binding regressions.
use std::marker::PhantomData;
use timetuck::*;
fn profile() -> PreparedAcceptors<SimpleScore> {
    let tabu = TabuSettings {
        hard: 1,
        fading: Some(1),
        aspiration: true,
    };
    AcceptorProfile {
        kinds: AcceptorKind::ALL.to_vec(),
        variable_tabu: Some(tabu),
        value_tabu: Some(tabu),
        move_tabu: Some(tabu),
        late_size: Some(10),
        step_period: Some(1),
        step_mode: StepCountingMode::ImprovingStep,
        temperature: Some(AnnealingTemperature::Loss(SimpleScore::new(-10))),
        seed: 33,
        ..Default::default()
    }
    .prepare(true)
    .unwrap()
}
fn manual() -> [Acceptance; 9] {
    [
        Acceptance::HillClimbing,
        Acceptance::StepCountingHillClimbing {
            period: 1,
            mode: StepCountingMode::ImprovingStep,
        },
        Acceptance::FadingVariableTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            aspiration: true,
            seed: 33,
        },
        Acceptance::FadingValueTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            aspiration: true,
            seed: 33,
        },
        Acceptance::FadingMoveTabu {
            hard_tenure: 1,
            fading_tenure: 1,
            aspiration: true,
            seed: 33,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(10),
            cooling_steps: 1000,
            seed: 33,
        },
        Acceptance::LateAcceptance {
            history_size: 10,
            hill_climbing: true,
        },
        Acceptance::DiversifiedLateAcceptance { history_size: 10 },
        Acceptance::GreatDeluge {
            initial_water_level: None,
            rise: GreatDelugeRise::Ratio {
                numerator: 1,
                denominator: 100000,
            },
        },
    ]
}
fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 3,
        selection: MoveSelection::BestAccepted,
        ..Default::default()
    }
}
trait Objective {
    fn score(&self) -> SimpleScore;
}
impl Objective for Assignment<i32> {
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.values().iter().flatten().map(|n| i64::from(*n)).sum())
    }
}
impl Objective for ListAssignment<i32> {
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.lists()[1].len() as i64)
    }
}
impl Objective for MixedAssignment<i32, i32> {
    fn score(&self) -> SimpleScore {
        self.basic().score() + self.lists().score()
    }
}
struct Calculator<C> {
    score: SimpleScore,
    marker: PhantomData<C>,
}
fn calculator<C>() -> Calculator<C> {
    Calculator {
        score: SimpleScore::ZERO,
        marker: PhantomData,
    }
}
impl<A: Objective, C> IncrementalCalculator<A> for Calculator<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.score();
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.score
    }
}
#[test]
fn prepared_composite_matches_manual_policies_in_all_nine_real_solver_backends() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(2, 2);
    ln.add_constraint("owner", vec![ListScope::Entity(1)], |v| {
        let ListFact::Entity(list) = v[0] else {
            unreachable!()
        };
        SimpleScore::new(list.len() as i64)
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 2, 2);
    mn.add_constraint(
        "both",
        vec![MixedScope::Basic(0), MixedScope::ListEntity(1)],
        |v| {
            let MixedFact::Basic(b) = v[0] else {
                unreachable!()
            };
            let MixedFact::ListEntity(l) = v[1] else {
                unreachable!()
            };
            SimpleScore::new(i64::from(b.copied().unwrap_or(0)) + l.len() as i64)
        },
    )
    .unwrap();
    let prepared = profile();
    let manual = manual();
    assert_eq!(prepared.policies(), &manual);
    macro_rules! run {
        ($solve:ident,$p:expr,$resource:expr,$expected:expr) => {{
            let mut observations = vec![];
            // Repeated prepared use must recreate fresh history/tabu/RNG state.
            for bound in [false, true, true] {
                let c = if bound {
                    prepared.bind(config())
                } else {
                    LocalSearchConfig {
                        acceptance: manual[0],
                        additional_acceptance: &manual[1..],
                        ..config()
                    }
                };
                let plan = SolverSequence {
                    phases: vec![SolverPhase::LocalSearch(c)],
                    ..Default::default()
                };
                let r = $solve(&$p, $resource, plan, SolverExtensions::default()).unwrap();
                assert_eq!(r.score, Some(SimpleScore::new($expected)));
                assert!(r.complete);
                observations.push((r.assignment, r.score, r.work, r.phases));
            }
            assert_eq!(observations[0], observations[1]);
            assert_eq!(observations[1], observations[2]);
        }};
    }
    macro_rules! mixed {
        ($solve:ident,$resource:expr) => {{
            let mut observations = vec![];
            for bound in [false, true, true] {
                let c = MixedLocalSearchConfig {
                    search: config(),
                    joint_moves: true,
                };
                let c = if bound {
                    prepared.bind_mixed(c)
                } else {
                    MixedLocalSearchConfig {
                        search: LocalSearchConfig {
                            acceptance: manual[0],
                            additional_acceptance: &manual[1..],
                            ..c.search
                        },
                        ..c
                    }
                };
                let r = $solve(
                    &m,
                    $resource,
                    MixedSolverSequence {
                        phases: vec![MixedSolverPhase::LocalSearch(c)],
                        ..Default::default()
                    },
                    SolverExtensions::default(),
                )
                .unwrap();
                assert_eq!(r.score, Some(SimpleScore::new(4)));
                assert!(r.complete);
                observations.push((r.assignment, r.score, r.work, r.phases));
            }
            assert_eq!(observations[0], observations[1]);
            assert_eq!(observations[1], observations[2]);
        }};
    }
    run!(solve_with_extensions, b, &bn, 2);
    run!(solve_list_with_extensions, l, &ln, 2);
    mixed!(solve_mixed_with_extensions, &mn);
    run!(
        solve_calculator_with_extensions,
        b,
        &|| calculator::<Vec<usize>>(),
        2
    );
    run!(
        solve_list_calculator_with_extensions,
        l,
        &|| calculator::<Vec<usize>>(),
        2
    );
    mixed!(solve_mixed_calculator_with_extensions, &|| calculator::<
        MixedCalculatorChange,
    >());
    run!(
        solve_stream_with_extensions,
        b,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = g.source::<usize, (usize, Option<i32>)>();
            g.score("value", s.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap_or(0)))
            })?;
            Ok((g.build(), s))
        },
        2
    );
    run!(
        solve_list_stream_with_extensions,
        l,
        || {
            let mut g = StreamGraphBuilder::new();
            let s = ListStreamSources::<i32>::new(&mut g);
            g.score("owner", s.values.port(), |v| {
                SimpleScore::new(i64::from(v.position.is_some_and(|p| p.entity == 1)))
            })?;
            Ok((g.build(), s))
        },
        2
    );
    mixed!(solve_mixed_stream_with_extensions, || {
        let mut g = StreamGraphBuilder::new();
        let s = MixedStreamSources::<i32, i32>::new(&mut g);
        g.score("value", s.basic.port(), |(_, v)| {
            SimpleScore::new(i64::from(v.unwrap_or(0)))
        })?;
        g.score("owner", s.lists.values.port(), |v| {
            SimpleScore::new(i64::from(v.position.is_some_and(|p| p.entity == 1)))
        })?;
        Ok((g.build(), s))
    });
}
#[test]
fn invalid_losses_and_sizes_do_not_mutate_configuration() {
    for loss in [
        SimpleScore::new(i64::MIN),
        SimpleScore::new(1),
        SimpleScore::new(-1).with_structural(-1),
    ] {
        let p = AcceptorProfile {
            temperature: Some(AnnealingTemperature::Loss(loss)),
            ..Default::default()
        };
        assert!(p.prepare(true).is_err());
        assert_eq!(p.temperature, Some(AnnealingTemperature::Loss(loss)));
    }
    for n in [-1, 0] {
        assert!(
            AcceptorProfile::<SimpleScore> {
                late_size: Some(n),
                ..Default::default()
            }
            .prepare(true)
            .is_err()
        );
        assert!(
            AcceptorProfile::<SimpleScore> {
                step_period: Some(n),
                ..Default::default()
            }
            .prepare(true)
            .is_err()
        );
    }
}
#[test]
fn canonical_order_deduplicates_requests_and_binding_preserves_unrelated_controls() {
    let p = AcceptorProfile::<SimpleScore> {
        kinds: vec![
            AcceptorKind::LateAcceptance,
            AcceptorKind::HillClimbing,
            AcceptorKind::LateAcceptance,
        ],
        late_size: Some(3),
        ..Default::default()
    }
    .prepare(true)
    .unwrap();
    assert!(matches!(
        p.policies(),
        [
            Acceptance::HillClimbing,
            Acceptance::LateAcceptance {
                history_size: 3,
                ..
            }
        ]
    ));
    let c = LocalSearchConfig {
        max_evaluations: 31,
        max_steps: 7,
        assert_step_scores: true,
        ..Default::default()
    };
    let bound = p.bind(c.clone());
    assert_eq!(
        (
            bound.max_evaluations,
            bound.max_steps,
            bound.assert_step_scores
        ),
        (c.max_evaluations, c.max_steps, c.assert_step_scores)
    );
    assert_eq!(bound.additional_acceptance, &p.policies()[1..]);
}

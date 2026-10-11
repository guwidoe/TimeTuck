//! Original retained-baseline/solver/cooling contracts; no upstream source credit.
use timetuck::*;
fn local(scope: TerminationProgressScope, temperature: i64, steps: u64) -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: steps,
        acceptance: Acceptance::SimulatedAnnealingTermination {
            starting_temperature: SimpleScore::new(temperature),
            scope,
            seed: 11,
        },
        ..Default::default()
    })
}
#[test]
fn complete_construction_initialization_seeds_solve_but_phase_rebases_after_handoff() {
    let p =
        FiniteProblem::new(vec![Variable::new(vec![100_i64, 80]).allowing_unassigned()]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    for phase in [false, true] {
        let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(SimpleScore::new(200), vec![]).unwrap(),
        ));
        let bindings = [PhaseTerminationBinding::stateful(1, &condition)];
        let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(80),
            }]
        };
        let mut ext = SolverExtensions {
            stateful_termination: (!phase).then_some(&condition),
            phase_terminations: if phase { &bindings } else { &[] },
            ..Default::default()
        };
        ext.move_providers.bind(1, &mut provider);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig {
                        optional: OptionalConstruction::NonWorsening,
                        ..Default::default()
                    }),
                    local(
                        if phase {
                            TerminationProgressScope::Phase
                        } else {
                            TerminationProgressScope::Solve
                        },
                        20,
                        1,
                    ),
                ],
                ..Default::default()
            },
            ext,
        )
        .unwrap();
        assert_eq!(r.work.construction_steps, 1);
        assert_eq!(r.work.completed_steps, u64::from(phase));
        assert_eq!(r.score, Some(SimpleScore::new(100)));
        assert_eq!(r.assignment.values(), &[Some(100)]);
    }
}
#[test]
fn construction_can_worsen_its_complete_initial_state_without_a_phantom_retained_goal() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64]).allowing_unassigned()]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
    n.add_constraint("unassigned bonus", vec![0], |v| {
        SimpleScore::new(if v[0].is_none() { 200 } else { 0 })
    })
    .unwrap();
    let count = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::ConstructionSteps(1),
    ));
    let children = [
        StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(SimpleScore::new(100), vec![]).unwrap(),
        )),
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&count)),
    ];
    let condition = StatefulSearchTermination::All(&children);
    for phase in [false, true] {
        let bindings = [PhaseTerminationBinding::stateful(0, &condition)];
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    optional: OptionalConstruction::EvaluateUnassigned,
                    pick_early: ConstructionPickEarly::FirstFeasibleScore,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                stateful_termination: (!phase).then_some(&condition),
                phase_terminations: if phase { &bindings } else { &[] },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::PlanCompleted);
        assert_eq!(r.work.construction_steps, 1);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(r.assignment.values(), &[Some(0)]);
    }
}
#[test]
fn partial_scores_cannot_seed_progress_and_best_survives_worsening_trials() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2]); 2]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(2);
    n.add_constraint("partial bonus", vec![0, 1], |v| {
        SimpleScore::new(if v.iter().any(Option::is_none) {
            1000
        } else {
            v.iter().flatten().map(|v| **v).sum()
        })
    })
    .unwrap();
    let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::target(SimpleScore::new(4), vec![]).unwrap(),
    ));
    let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, step: u64| {
        vec![Move::Change {
            variable: 0,
            value: Some(if step == 0 { 2 } else { 1 }),
        }]
    };
    let mut ext = SolverExtensions {
        stateful_termination: Some(&condition),
        ..Default::default()
    };
    ext.move_providers.bind(1, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig {
                    pick_early: ConstructionPickEarly::FirstFeasibleScore,
                    ..Default::default()
                }),
                local(TerminationProgressScope::Solve, 1, 2),
            ],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.phases[0].score, Some(SimpleScore::new(2)));
    assert_eq!(
        r.work.completed_steps, 1,
        "initialized start2 -> best3 yields .5, rejecting the seeded worsening trial"
    );
    assert_eq!(r.score, Some(SimpleScore::new(3)));
    assert_eq!(r.assignment.values(), &[Some(2), Some(1)]);
}
#[test]
fn rounded_or_zero_weight_progress_never_substitutes_for_exact_feasibility() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<_, BendableScore<2, 1>>::new(1);
    n.add_constraint("hard levels", vec![0], |v| match *v[0].unwrap() {
        0 => BendableScore::new([-10, -10], [0]),
        1 => BendableScore::new([0, -1], [100]),
        _ => BendableScore::new([0, 0], [-100]),
    })
    .unwrap();
    let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
        ScoreGoalProgressConfig::<BendableScore<2, 1>>::feasibility(vec![1.0]).unwrap(),
    ));
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        BendableScore<2, 1>,
        ScoringError,
    >| {
        c.execute(&Move::Change {
            variable: 0,
            value: Some(1),
        })?
        .unwrap();
        assert!(
            !c.is_terminated(),
            "progress1 with a remaining hard violation is not feasible"
        );
        c.execute(&Move::Change {
            variable: 0,
            value: Some(2),
        })?
        .unwrap();
        assert!(c.is_terminated());
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            stateful_termination: Some(&condition),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 2);
    assert!(r.score.unwrap().is_feasible());
}
#[test]
fn bounded_and_enumerated_exact_search_ignore_partial_roots_then_stop_on_target() {
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::BranchAndBound(Default::default()),
    ] {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2]); 2]).unwrap();
        let mut n = ConstraintNetwork::<_, SimpleScore>::new(2);
        n.add_constraint("partial bonus", vec![0, 1], |v| {
            SimpleScore::new(if v.iter().any(Option::is_none) {
                1000
            } else {
                v.iter().flatten().map(|v| **v).sum()
            })
        })
        .unwrap();
        let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::ScoreGoal(
            ScoreGoalProgressConfig::target(SimpleScore::new(3), vec![]).unwrap(),
        ));
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                stateful_termination: Some(&condition),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::Condition);
        assert!(r.complete);
        assert!(r.score.unwrap() >= SimpleScore::new(3));
        assert!(!r.is_optimal());
    }
}

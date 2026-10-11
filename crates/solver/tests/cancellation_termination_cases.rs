// TerminationTest and its argument/scoring fixtures adapted from pinned Timefold
// Community under Apache-2.0; see compat/provenance.json. No production source used.
use std::{cell::Cell, time::Duration};
use timetuck::*;

// Full TerminationArgumentSource order; no row is assumed away or skipped.
fn with_policy<R>(
    row: usize,
    construction: bool,
    token: &CancellationToken,
    run: impl FnOnce(SolverSequence<'_, SimpleScore>) -> R,
) -> R {
    let children = [
        SearchTermination::Condition(SearchLimit::CombinedSteps(10_000)),
        SearchTermination::Condition(SearchLimit::MoveEvaluations(10_000)),
    ];
    let expression = match row {
        0 => Some(children[0]),
        1 => Some(SearchTermination::Condition(SearchLimit::ScoreEvaluations(
            10_000,
        ))),
        2 | 4 => None,
        3 => Some(SearchTermination::Condition(SearchLimit::Time(
            Duration::from_secs(10 * 24 * 60 * 60),
        ))),
        5 => Some(SearchTermination::Condition(SearchLimit::UnimprovedTime(
            Duration::from_secs(10 * 24 * 60 * 60),
        ))),
        6 => Some(SearchTermination::Condition(SearchLimit::UnimprovedSteps(
            10_000,
        ))),
        7 => Some(children[1]),
        8 => Some(SearchTermination::Any(&children)),
        9 => Some(SearchTermination::All(&children)),
        _ => unreachable!(),
    };
    let mut plan = SolverSequence {
        limits: SolverLimits {
            max_evaluations: u64::MAX,
            cancellation: Some(token),
            termination: expression.as_ref(),
            best_score_limit: (row == 2).then_some(SimpleScore::new(1_000)),
            ..Default::default()
        },
        phases: vec![if construction {
            SolverPhase::Construction(ConstructionConfig {
                max_evaluations: u64::MAX,
                ..Default::default()
            })
        } else {
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: u64::MAX,
                max_steps: u64::MAX,
                max_accepted_moves: u64::MAX,
                ..Default::default()
            })
        }],
    };
    if row == 4 {
        // Community ignores diminished returns during construction. Native
        // defaults explicitly apply only to local-search phases: zero applicable
        // phases here is the mapping, not solve-wide diminished-return support.
        assert_eq!(
            apply_diminished_returns_default(&mut plan, default_diminished_returns_config()),
            usize::from(!construction)
        );
    }
    run(plan)
}
fn initial(assigned: bool) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..2)
            .map(|i| {
                let variable = Variable::new(vec![0, 1]);
                if assigned {
                    variable.with_initial(i)
                } else {
                    variable
                }
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn terminate_early_construction_heuristic() {
    for row in 0..10 {
        let problem = initial(false);
        let token = CancellationToken::new();
        let initial_scores = Cell::new(0);
        let requests = Cell::new(0);
        let mut network = ConstraintNetwork::<i32, SimpleScore>::new(2);
        // DummySimpleScoreThrowingEasyScoreCalculator: allow the initial state,
        // terminate while exactly one entity is assigned, and fail if complete
        // construction is ever scored. The native handle always exists, so the
        // source's assumeTrue(nonnull solver handle) cannot skip any matrix row.
        network
            .add_constraint("termination scorer", vec![0, 1], |values| {
                match values.iter().filter(|value| value.is_none()).count() {
                    2 => initial_scores.set(initial_scores.get() + 1),
                    1 => {
                        requests.set(requests.get() + 1);
                        token.cancel();
                    }
                    _ => panic!("Expected Construction Heuristic to terminate early"),
                }
                SimpleScore::ZERO
            })
            .unwrap();
        let result = with_policy(row, true, &token, |plan| {
            solve_with_observer(&problem, &network, plan, |_| {})
        })
        .unwrap();
        assert!(
            result.score.is_some(),
            "row {row}: returned solution must have a score"
        );
        assert!(
            initial_scores.get() > 0,
            "row {row}: missing initial scoring"
        );
        assert!(
            requests.get() > 0,
            "row {row}: termination callback did not fire"
        );
        assert!(token.is_cancelled());
        assert_eq!(result.termination, SolverTermination::Cancelled);
        assert!(!result.complete);
    }
}

#[test]
fn terminate_early_local_search() {
    for row in 0..10 {
        let problem = initial(true);
        let token = CancellationToken::new();
        let requests = Cell::new(0);
        let mut network = ConstraintNetwork::<i32, SimpleScore>::new(2);
        // TestdataConstraintProviderNonzeroValues: generated value code ending
        // in 0 contains zero; penalize each assigned entity using that value.
        network
            .add_constraint("Value contains zero", vec![0, 1], |values| {
                SimpleScore::new(
                    -(values
                        .iter()
                        .filter(|value| value.is_some_and(|v| *v == 0))
                        .count() as i64),
                )
            })
            .unwrap();
        let result = with_policy(row, false, &token, |plan| {
            solve_with_observer(&problem, &network, plan, |event| {
                assert_eq!(event.kind, SolverPhaseKind::LocalSearch);
                requests.set(requests.get() + 1);
                token.cancel();
            })
        })
        .unwrap();
        assert!(
            result.score.is_some(),
            "row {row}: returned solution must exist"
        );
        assert!(requests.get() > 0, "row {row}: best listener did not fire");
        assert!(token.is_cancelled());
        assert_eq!(result.termination, SolverTermination::Cancelled);
        assert_eq!(result.score, Some(SimpleScore::new(-1)));
    }
}

#[test]
fn mixed_solver_phase_terminations() {
    let problem = initial(true);
    let mut network = ConstraintNetwork::<i32, SimpleScore>::new(2);
    // TestdataEasyScoreCalculator, all ordered unequal-entity pairs.
    network
        .add_constraint("duplicates", vec![0, 1], |values| {
            SimpleScore::new(if values[0].is_some() && values[0] == values[1] {
                -2
            } else {
                0
            })
        })
        .unwrap();
    let condition = SearchTermination::Condition(SearchLimit::Steps(4));
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    let result = solve_with_extensions(
        &problem,
        &network,
        SolverSequence {
            limits: SolverLimits {
                max_time: Some(Duration::from_secs(60 * 60)),
                ..Default::default()
            },
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: u64::MAX,
                max_accepted_moves: u64::MAX,
                acceptance: Acceptance::HillClimbing,
                neighborhood: Neighborhood::ChangeAndSwap,
                ..Default::default()
            })],
        },
        SolverExtensions {
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.score.is_some());
    assert_eq!(result.work.completed_steps, 4);
    assert_eq!(result.phases[0].work.completed_steps, 4);
    assert_eq!(
        result.phases[0].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
    );
    assert_eq!(result.termination, SolverTermination::PlanCompleted);
}

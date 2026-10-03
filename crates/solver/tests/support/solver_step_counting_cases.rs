//! Original integration across model/backend pairs and ordered phase plans.
use super::*;
const MODES: [StepCountingMode; 5] = [
    StepCountingMode::Step,
    StepCountingMode::EqualOrImprovingStep,
    StepCountingMode::ImprovingStep,
    StepCountingMode::SelectedMove,
    StepCountingMode::AcceptedMove,
];
fn score(v: i64) -> SimpleScore {
    SimpleScore::new([0, -4, -1, -3, 5][v as usize])
}
#[test]
fn step_counting_works_across_all_models_backends_and_phase_handoffs() {
    for mode in MODES {
        for period in [1, 2, 7] {
            for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
                for model in 0usize..3 {
                    let c = SolverPlan {
                        phases: vec![
                            SolverPhase::LocalSearch(LocalSearchConfig {
                                acceptance: Acceptance::StepCountingHillClimbing { period, mode },
                                max_steps: 12,
                                max_evaluations: 1000,
                                selection,
                                rejected_moves: RejectedMovePolicy::BestScore,
                                assert_scores: true,
                                ..Default::default()
                            });
                            2
                        ],
                        ..Default::default()
                    };
                    let run = |backend: usize| {
                        run_scored(
                            (
                                model * 2 + backend,
                                true,
                                MixedConstructionOrder::BasicThenLists,
                            ),
                            c.clone(),
                            &Cell::new(0),
                            None,
                            || {},
                            score,
                        )
                        .unwrap()
                    };
                    let a = run(0);
                    let b = run(1);
                    assert_eq!(a.values, b.values);
                    assert_eq!(a.score, b.score);
                    assert_eq!(a.work.evaluations, b.work.evaluations);
                    assert_eq!(a.work.move_evaluations, b.work.move_evaluations);
                    assert_eq!(a.work.completed_steps, 24);
                    assert_eq!(b.work.completed_steps, 24);
                    assert_eq!(a.work.accepted_moves, b.work.accepted_moves);
                    assert_eq!(a.reason, SolverTermination::PlanCompleted);
                    assert_eq!(b.reason, a.reason);
                    assert_eq!(a.phases.len(), 3);
                    assert_eq!(b.phases.len(), 3);
                    assert!(a.complete && b.complete);
                    assert!(a.score.unwrap() >= SimpleScore::ZERO);
                    for (x, y) in a.phases.iter().zip(&b.phases) {
                        assert_eq!(x.score, y.score);
                        assert_eq!(x.termination, y.termination);
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_step_counting_phase_is_rejected_even_after_an_exact_proof_would_skip_it() {
    for mode in 0..6 {
        let c = SolverPlan {
            phases: vec![
                SolverPhase::Exact(ExactPhaseConfig::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    acceptance: Acceptance::StepCountingHillClimbing {
                        period: 0,
                        mode: StepCountingMode::Step,
                    },
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        assert_eq!(
            run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || panic!("validation must precede callbacks"),
                score
            ),
            Err(Failure::Acceptance(2))
        );
    }
}

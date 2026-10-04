//! Original solver integration checks; no additional upstream method credit.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
use std::time::Duration;

#[test]
fn composed_limits_span_phases_in_all_six_paths() {
    for mode in 0..6 {
        let children = [
            SearchTermination::Condition(SearchLimit::Steps(3)),
            SearchTermination::Condition(SearchLimit::UnimprovedSteps(5)),
        ];
        for (condition, wanted, phase_count) in [
            (SearchTermination::Any(&children), 3, 2),
            (SearchTermination::All(&children), 5, 3),
        ] {
            let r = run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&condition),
                        ..Default::default()
                    },
                    phases: vec![
                        SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 2,
                            acceptance: Acceptance::HillClimbing,
                            selection: MoveSelection::FirstAccepted,
                            ..Default::default()
                        });
                        4
                    ],
                },
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO,
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
            assert_eq!(r.work.completed_steps, wanted);
            assert_eq!(r.phases.len(), phase_count);
            assert!(r.complete);
            assert_eq!(r.score, Some(SimpleScore::ZERO));
        }
    }
}

#[test]
fn zero_expression_stops_before_any_scoring_or_factory() {
    let condition = SearchTermination::Condition(SearchLimit::Time(Duration::ZERO));
    for mode in 0..6 {
        let factories = Cell::new(0);
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            },
            &factories,
            None,
            || panic!("must not score"),
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Condition);
        assert!(r.phases.is_empty());
        assert_eq!(r.work.evaluations, 0);
        assert_eq!(factories.get(), 0);
    }
}

#[test]
fn nested_progress_keeps_applicability_separate_from_unsupported_estimates() {
    use TerminationExpression as E;
    let inner = [E::Condition(0), E::Condition(1)];
    let outer = [E::All(&inner), E::Condition(2)];
    let expr = E::Any(&outer);
    let mut visits = vec![];
    let state = expr
        .evaluate(|&i| {
            visits.push(i);
            match i {
                0 => Some(TerminationState {
                    reached: false,
                    progress: None,
                }),
                1 => None,
                _ => Some(TerminationState {
                    reached: true,
                    progress: Some(0.25),
                }),
            }
        })
        .unwrap();
    assert_eq!(visits, [0, 1, 2]);
    assert!(state.reached);
    assert_eq!(state.progress, Some(1.0)); // AND's unsupported-gradient identity.
    for invalid in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(
            E::Condition(())
                .evaluate(|_| Some(TerminationState {
                    reached: false,
                    progress: Some(invalid)
                }))
                .unwrap()
                .progress,
            None
        );
    }
}

#[test]
fn exact_integer_threshold_does_not_use_rounded_progress() {
    let limit = SearchLimit::Steps(u64::MAX);
    let state = limit
        .evaluate(SearchProgress {
            completed_steps: u64::MAX - 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(state.progress, Some(1.0));
    assert!(!state.reached);
    assert!(
        limit
            .evaluate(SearchProgress {
                completed_steps: u64::MAX,
                ..Default::default()
            })
            .unwrap()
            .reached
    );
}

#[test]
fn repair_stops_within_the_phase_in_all_six_paths() {
    let condition = SearchTermination::Condition(SearchLimit::Steps(1));
    for mode in 0..6 {
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::RuinRecreate(RuinRecreateConfig {
                        minimum_ruined: usize::MAX,
                        maximum_ruined: usize::MAX,
                        max_attempts: 50,
                        ..Default::default()
                    });
                    2
                ],
            },
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Condition);
        assert_eq!(r.work.completed_steps, 1);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::RuinRecreate(RuinRecreateTermination::Condition)
        );
    }
}

#[test]
fn unstarted_stagnation_does_not_end_an_and_expression() {
    let children = [
        SearchTermination::Condition(SearchLimit::UnimprovedSteps(0)),
        SearchTermination::Condition(SearchLimit::UnimprovedTime(Duration::ZERO)),
    ];
    let condition = SearchTermination::All(&children);
    assert!(!condition.status(SearchProgress::default()).unwrap().reached);
    assert!(
        condition
            .status(SearchProgress {
                unimproved_steps: Some(0),
                unimproved_time: Some(Duration::ZERO),
                ..Default::default()
            })
            .unwrap()
            .reached
    );
}

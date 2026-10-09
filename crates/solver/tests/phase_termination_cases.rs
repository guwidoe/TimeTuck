// Adapted from Community TerminationTest and TestdataEasyScoreCalculator.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;
fn fixture() -> (
    FiniteProblem<usize>,
    ConstraintNetwork<'static, usize, SimpleScore>,
) {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("duplicate ordered pairs", vec![0, 1], |values| {
        let mut score = 0;
        for (i, value) in values.iter().enumerate() {
            if value.is_some() {
                for (j, other) in values.iter().enumerate() {
                    if i != j && value == other {
                        score -= 1;
                    }
                }
            }
        }
        SimpleScore::new(score)
    })
    .unwrap();
    (p, n)
}
fn profile(
    steps: Option<i64>,
    time: Option<Duration>,
    window: Option<Duration>,
) -> TerminationProfile {
    TerminationProfile::new(None, time, steps, None, window, None).unwrap()
}
#[test]
fn step_count_at_solver_level() {
    let (p, n) = fixture();
    let stop = SearchTermination::Condition(SearchLimit::CombinedSteps(1));
    let r = solve(
        &p,
        &n,
        SolverConfig {
            limits: SolverLimits {
                termination: Some(&stop),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.score.unwrap().with_structural(0), SimpleScore::ZERO);
    assert!(!r.complete);
    assert_eq!(r.work.construction_steps, 1);
}
#[test]
fn step_count_at_phase_level() {
    let (p, n) = fixture();
    let r = solve(
        &p,
        &n,
        SolverConfig {
            construction: profile(Some(1), None, None)
                .bind_construction(ConstructionConfig::default())
                .unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.score.unwrap().with_structural(0), SimpleScore::ZERO);
    assert!(!r.complete);
    assert_eq!(r.work.construction_steps, 1);
}
#[test]
fn diminished_returns_at_solver_level() {
    let (p, n) = fixture();
    let r = profile(
        None,
        None,
        Some(default_diminished_returns_config().window()),
    )
    .with_sequence(SolverConfig::default(), |sequence| solve(&p, &n, sequence))
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert!(r.complete);
}
#[test]
fn diminished_returns_inapplicable_at_construction_phase() {
    let error = profile(
        None,
        None,
        Some(default_diminished_returns_config().window()),
    )
    .bind_construction(ConstructionConfig::default())
    .unwrap_err();
    assert_eq!(
        error,
        TerminationProfileError::InapplicableToConstruction {
            setting: "DiminishedReturns"
        }
    );
    assert!(error.to_string().contains("not applicable"));
    assert!(error.to_string().contains("DiminishedReturns"));
}
#[test]
fn unimproved_time_inapplicable_at_construction_phase() {
    let error = profile(None, Some(Duration::from_millis(1)), None)
        .bind_construction(ConstructionConfig::default())
        .unwrap_err();
    assert_eq!(
        error,
        TerminationProfileError::InapplicableToConstruction {
            setting: "UnimprovedTimeMillisSpent"
        }
    );
    assert!(error.to_string().contains("not applicable"));
    assert!(error.to_string().contains("UnimprovedTimeMillisSpent"));
}

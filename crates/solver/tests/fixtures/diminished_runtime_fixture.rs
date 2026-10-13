// Adapted TEST setup and helper values; Apache-2.0, see THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;
pub const WINDOW: Duration = Duration::from_secs(30 * 60);
pub const RATIO: f64 = 123.0;
pub fn properties(raw: RuntimeDiminishedReturns) -> RuntimeSolverProperties {
    RuntimeSolverProperties {
        termination: Some(RuntimeTerminationProperties {
            diminished_returns: Some(raw),
        }),
    }
}
pub fn empty() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![],
        ..Default::default()
    }
}
pub fn assert_no(
    binding: DiminishedRuntimeBinding,
    plan: &SolverSequence<'_>,
    before: &SolverSequence<'_>,
) {
    assert!(binding.termination.diminished_returns.is_none());
    assert!(binding.resolved.is_none());
    assert_eq!(binding.applied_local_phases, 0);
    // Source phase-null helper: no runtime termination introduced into any
    // construction/local phase. Full equality also checks every native control.
    assert_eq!(plan, before);
    for phase in &plan.phases {
        match phase {
            SolverPhase::Construction(config) => assert_eq!(config, &ConstructionConfig::default()),
            SolverPhase::LocalSearch(config) => assert!(config.diminished_returns.is_none()),
            _ => panic!("source fixture has only construction/local phases"),
        }
    }
}
pub fn assert_present(
    binding: DiminishedRuntimeBinding,
    window: Option<Duration>,
    ratio: Option<f64>,
) {
    let raw = binding.termination.diminished_returns.unwrap();
    assert_eq!(raw.sliding_window_duration, window);
    assert_eq!(raw.minimum_improvement_ratio, ratio);
    let actual = binding.resolved.unwrap();
    assert_eq!(
        actual.window(),
        window.unwrap_or(default_diminished_returns_config().window())
    );
    assert_eq!(
        actual.ratio(),
        if ratio.is_some() {
            (123, 1)
        } else {
            default_diminished_returns_config().ratio()
        }
    );
}

use std::time::Duration;
use timetuck::*;
fn resolve(pairs: &[(&str, &str)]) -> Result<RuntimePropertyPlan, RuntimePropertyPlanError> {
    RuntimePropertyPlan::resolve(
        &OwnedPropertySource::new(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string()))),
        &PropertyFallbackRules::community_test_profile(),
        "timefold.model.termination",
        &mut |_| {},
    )
}
#[test]
fn durations_limits_and_explicit_policies_are_preserved_transactionally() {
    for (raw, seconds) in [
        ("30s", 30),
        ("5m", 300),
        ("2h", 7200),
        ("1d", 86400),
        ("PT30S", 30),
        ("P1D", 86400),
    ] {
        assert_eq!(
            parse_runtime_property_duration("key", raw).unwrap(),
            Duration::from_secs(seconds)
        );
    }
    assert_eq!(
        parse_runtime_property_duration("key", "15ms").unwrap(),
        Duration::from_millis(15)
    );
    let explicit = DiminishedReturnsConfig::new(Duration::from_secs(7), 3, 4).unwrap();
    let mut p = SolverSequence::<SimpleScore>::default();
    p.limits.max_evaluations = 77;
    if let SolverPhase::LocalSearch(c) = &mut p.phases[1] {
        c.diminished_returns = Some(explicit);
        c.move_order = MoveOrder::ShuffledVariables { seed: 37 };
    }
    let phases = p.phases.clone();
    let raw = resolve(&[
        ("timefold.model.termination.spent-limit", "PT20S"),
        ("timefold.model.termination.unimproved-spent-limit", "2s"),
        (
            "timefold.model.termination.diminished-returns.enabled",
            "true",
        ),
    ])
    .unwrap();
    assert_eq!(raw.bind(&mut p).unwrap().applied_local_phases, 0);
    assert_eq!(p.phases, phases);
    assert_eq!(p.limits.max_time, Some(Duration::from_secs(20)));
    assert_eq!(p.limits.max_unimproved_time, Some(Duration::from_secs(2)));
    assert_eq!(p.limits.max_evaluations, 77);
    let before = p.clone();
    resolve(&[]).unwrap().bind(&mut p).unwrap();
    assert_eq!(p, before);
    let mut mixed = MixedSolverSequence::<SimpleScore>::default();
    let before = mixed.clone();
    let raw = resolve(&[
        (
            "timefold.model.termination.diminished-returns.minimum-improvement-ratio",
            "1e400",
        ),
        ("timefold.model.termination.spent-limit", "1s"),
    ]);
    assert!(raw.is_err());
    let raw = RuntimePropertyPlan {
        spent_limit: Some(Duration::from_secs(1)),
        solver: RuntimeSolverProperties {
            termination: Some(RuntimeTerminationProperties {
                diminished_returns: Some(RuntimeDiminishedReturns {
                    minimum_improvement_ratio: Some(f64::INFINITY),
                    ..Default::default()
                }),
            }),
        },
        ..Default::default()
    };
    assert!(raw.bind_mixed(&mut mixed).is_err());
    assert_eq!(mixed, before);
    let before = p.clone();
    assert!(raw.bind(&mut p).is_err());
    assert_eq!(p, before);
}
#[test]
fn malformed_overflow_and_alias_diagnostics_are_real() {
    for raw in [
        "",
        "-1s",
        "1.5s",
        "1w",
        " 1s",
        "18446744073709551616s",
        "18446744073709551615d",
        "PT-1S",
    ] {
        assert!(
            parse_runtime_property_duration("key", raw).is_err(),
            "{raw}"
        );
    }
    for (suffix, value) in [
        ("enabled", "TRUE"),
        ("minimum-improvement-ratio", "NaN"),
        ("minimum-improvement-ratio", "-1"),
        ("sliding-window-duration", "0s"),
    ] {
        assert!(
            resolve(&[(
                &format!("timefold.model.termination.diminished-returns.{suffix}"),
                value
            )])
            .is_err()
        );
    }
    let source = OwnedPropertySource::new([(
        "ai.timefold.platform.termination.spent-limit".into(),
        "4s".into(),
    )]);
    let mut warnings = Vec::new();
    let raw = RuntimePropertyPlan::resolve(
        &source,
        &PropertyFallbackRules::community_test_profile(),
        "timefold.model.termination",
        &mut |w| warnings.push(w),
    )
    .unwrap();
    assert_eq!(raw.spent_limit, Some(Duration::from_secs(4)));
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].legacy,
        "ai.timefold.platform.termination.spent-limit"
    );
}

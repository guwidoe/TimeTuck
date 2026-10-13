use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
fn source(pairs: &[(&str, &str)]) -> OwnedPropertySource {
    OwnedPropertySource::new(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())))
}
#[test]
fn longest_boundary_prefix_exact_override_and_ordered_aliases() {
    let rules = PropertyFallbackRules::new()
        .prefix("root", vec!["old".into()])
        .prefix("root.deep", vec!["first.deep".into(), "second.deep".into()])
        .exact("root.deep.exact", vec!["precise".into()]);
    let values = source(&[
        ("old.deep.value", "short"),
        ("first.deep.value", "long"),
        ("second.deep.value", "second"),
        ("precise", "exact"),
        ("first.deep.exact", "prefix"),
        ("oldish.value", "bad"),
    ]);
    let mut warnings = Vec::new();
    assert_eq!(
        rules.resolve(&values, "root.deep.value", &mut |w| warnings.push(w)),
        Some("long")
    );
    assert_eq!(
        rules.resolve(&values, "root.deep.exact", &mut |w| warnings.push(w)),
        Some("exact")
    );
    assert_eq!(
        rules.resolve(&values, "rootish.value", &mut |w| warnings.push(w)),
        None
    );
    assert_eq!(warnings.len(), 2);
    assert_eq!(warnings[0].legacy, "first.deep.value");
    assert_eq!(warnings[1].legacy, "precise");
    let canonical = source(&[("root.deep.value", ""), ("first.deep.value", "legacy")]);
    assert_eq!(
        rules.resolve(&canonical, "root.deep.value", &mut |w| warnings.push(w)),
        Some("")
    );
    assert_eq!(warnings.len(), 2);
    let second = source(&[("second.deep.value", "fallback")]);
    assert_eq!(
        rules.resolve(&second, "root.deep.value", &mut |w| warnings.push(w)),
        Some("fallback")
    );
    assert_eq!(warnings.last().unwrap().legacy, "second.deep.value");
    // A matched longer prefix does not accidentally try unrelated shorter rules.
    assert_eq!(
        rules.resolve(
            &source(&[("old.deep.value", "short")]),
            "root.deep.value",
            &mut |w| warnings.push(w)
        ),
        None
    );
}
#[test]
fn selected_live_reads_warn_without_eager_or_miss_diagnostics() {
    let rules = PropertyFallbackRules::community_test_profile();
    let values = source(&[
        ("timefold.application.id", "first"),
        ("ai.timefold.platform.model", "second"),
    ]);
    let mut warnings = Vec::new();
    assert!(warnings.is_empty());
    for _ in 0..2 {
        assert_eq!(
            rules.resolve(&values, "timefold.model.id", &mut |w| warnings.push(w)),
            Some("first")
        );
    }
    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .all(|w| w.canonical == "timefold.model.id" && w.legacy == "timefold.application.id")
    );
    assert_eq!(
        rules.resolve(&values, "missing", &mut |w| warnings.push(w)),
        None
    );
    assert_eq!(warnings.len(), 2);
}
#[test]
fn resolved_owned_configuration_executes_dispatcher_and_failure_never_enqueues() {
    let rules = PropertyFallbackRules::community_test_profile();
    for count in [1, 4, 6] {
        let mut supplied = vec![
            (
                "timefold.application.id".to_string(),
                format!("owned-{count}"),
            ),
            ("timefold.application.version".into(), format!("v{count}")),
            (
                "ai.timefold.platform.model-resource".into(),
                "/schedules".into(),
            ),
        ];
        let values = OwnedPropertySource::new(supplied.clone());
        supplied.clear();
        let mut warnings = Vec::new();
        let mut dispatcher = rules
            .employee_dispatcher(&values, 2, &mut |w| warnings.push(w))
            .unwrap();
        assert_eq!(warnings.len(), 3);
        let schedule = NativeEmployeeSchedule {
            employees: vec![ScheduleEmployee {
                id: "worker".into(),
                skills: BTreeSet::from(["skill".into()]),
            }],
            shifts: (0..count)
                .map(|i| EmployeeShift {
                    id: format!("shift-{i}"),
                    required_skill: "skill".into(),
                    start: i as i64 * 10,
                    end: i as i64 * 10 + 5,
                    employee: None,
                })
                .collect(),
        };
        let route = format!("/v{count}/schedules");
        let receipt = dispatcher
            .enqueue(
                "POST",
                &route,
                &BTreeMap::from([("priority".into(), count.to_string())]),
                EmployeeRouteCommand::Schedule(schedule),
            )
            .unwrap();
        assert_eq!(receipt.status_code, 202);
        assert_eq!(receipt.priority, count);
        let mut resource = EmployeeScheduleResource::new();
        let (actual, response) = dispatcher.execute_next(&mut resource).unwrap();
        assert_eq!(actual, receipt);
        let response = response.unwrap();
        assert_eq!(response.assigned_shifts(), count as usize);
        assert_eq!(response.score.medium(), 0);
        assert!(response.score.is_feasible());
        assert!(dispatcher.execute_next(&mut resource).is_none());
        assert_eq!(
            dispatcher.openapi()["paths"][&route]["post"]["operationId"],
            "schedule"
        );
    }
    let invalid = source(&[
        ("timefold.model.id", "valid"),
        ("timefold.model.api-version", "v1"),
        ("timefold.model.rest-resource", "/other"),
        ("timefold.application.id", "ignored"),
    ]);
    let mut warnings = Vec::new();
    assert_eq!(
        rules
            .employee_dispatcher(&invalid, 2, &mut |w| warnings.push(w))
            .err()
            .unwrap()
            .kind,
        ServiceDescriptorErrorKind::ResourceMismatch
    );
    assert!(warnings.is_empty());
    let missing = source(&[]);
    assert_eq!(
        rules
            .employee_dispatcher(&missing, 2, &mut |w| warnings.push(w))
            .err()
            .unwrap()
            .kind,
        ServiceDescriptorErrorKind::ModelId
    );
    assert!(warnings.is_empty());
}

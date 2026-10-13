// Adapted Community TEST assertions; Apache-2.0; see fixture NOTICE.
use timetuck::{OwnedPropertySource, PropertyFallbackRules};
#[path = "fixtures/property_fallback_fixture.rs"]
mod fixture;
#[test]
fn falls_back_to_exact_legacy_name() {
    let source =
        OwnedPropertySource::new(vec![("timefold.application.name".into(), "my-app".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.name", &mut |w| warnings.push(w)),
        Some("my-app")
    );
}
#[test]
fn prefers_new_name_over_legacy_for_exact_rename() {
    let source = OwnedPropertySource::new(vec![
        ("timefold.model.name".into(), "new-name".into()),
        ("timefold.application.name".into(), "legacy-name".into()),
    ]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.name", &mut |w| warnings.push(w)),
        Some("new-name")
    );
}
#[test]
fn falls_back_across_prefix_rename_platform() {
    let source =
        OwnedPropertySource::new(vec![("ai.timefold.platform.foo.bar".into(), "42".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.platform.foo.bar", &mut |w| warnings
            .push(w)),
        Some("42")
    );
}
#[test]
fn falls_back_across_prefix_rename_default_config() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.model.default-config.something".into(),
        "v".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(
            &source,
            "timefold.model.default-config.something",
            &mut |w| warnings.push(w)
        ),
        Some("v")
    );
}
#[test]
fn falls_back_across_prefix_rename_termination() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.platform.termination.spent-limit".into(),
        "10s".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(
            &source,
            "timefold.model.termination.spent-limit",
            &mut |w| warnings.push(w)
        ),
        Some("10s")
    );
}
#[test]
fn falls_back_across_prefix_rename_contact() {
    let source = OwnedPropertySource::new(vec![(
        "timefold.application.contact.email".into(),
        "a@b.c".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.contact.email", &mut |w| warnings
            .push(w)),
        Some("a@b.c")
    );
}
#[test]
fn falls_back_across_prefix_rename_rest() {
    let source = OwnedPropertySource::new(vec![("timefold.rest.path".into(), "/api".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.rest.path", &mut |w| warnings
            .push(w)),
        Some("/api")
    );
}
#[test]
fn falls_back_across_prefix_rename_storage() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.storage.type".into(),
        "in-memory".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.storage.type", &mut |w| warnings.push(w)),
        Some("in-memory")
    );
}
#[test]
fn falls_back_for_multiple_legacy_names_first_legacy() {
    let source = OwnedPropertySource::new(vec![("timefold.application.id".into(), "id-1".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.id", &mut |w| warnings.push(w)),
        Some("id-1")
    );
}
#[test]
fn falls_back_for_multiple_legacy_names_second_legacy() {
    let source =
        OwnedPropertySource::new(vec![("ai.timefold.platform.model".into(), "id-2".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.id", &mut |w| warnings.push(w)),
        Some("id-2")
    );
}
#[test]
fn new_name_wins_over_multiple_legacy_names() {
    let source = OwnedPropertySource::new(vec![
        ("timefold.model.id".into(), "new-id".into()),
        ("timefold.application.id".into(), "legacy-id-a".into()),
        ("ai.timefold.platform.model".into(), "legacy-id-b".into()),
    ]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.id", &mut |w| warnings.push(w)),
        Some("new-id")
    );
}
#[test]
fn api_version_resolves_from_legacy_version() {
    let source =
        OwnedPropertySource::new(vec![("timefold.application.version".into(), "v1".into())]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.api-version", &mut |w| warnings
            .push(w)),
        Some("v1")
    );
}
#[test]
fn api_version_resolves_from_legacy_model_version() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.platform.model-version".into(),
        "v1".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.api-version", &mut |w| warnings
            .push(w)),
        Some("v1")
    );
}
#[test]
fn rest_resource_legacy_rename() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.platform.model-resource".into(),
        "/models".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.rest-resource", &mut |w| warnings
            .push(w)),
        Some("/models")
    );
}
#[test]
fn schema_validation_legacy_rename() {
    let source = OwnedPropertySource::new(vec![(
        "ai.timefold.platform.models.validation.enable".into(),
        "true".into(),
    )]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(
            &source,
            "timefold.model.schema.validation.enable",
            &mut |w| warnings.push(w)
        ),
        Some("true")
    );
}
#[test]
fn unknown_property_returns_null() {
    let source = OwnedPropertySource::new(vec![]);
    let mut warnings = Vec::new();
    let rules = PropertyFallbackRules::community_test_profile();
    assert_eq!(
        rules.resolve(&source, "timefold.model.name", &mut |w| warnings.push(w)),
        None
    );
}
#[test]
fn logs_warning_when_legacy_property_is_defined() {
    for (canonical, legacy) in fixture::MAPPING_PAIRS {
        let source = OwnedPropertySource::new([(legacy.into(), "my-value".into())]);
        let mut warnings = Vec::new();
        PropertyFallbackRules::community_test_profile()
            .resolve(&source, canonical, &mut |w| warnings.push(w));
        assert!(
            warnings
                .iter()
                .any(|w| w.message.contains("Deprecated configuration key"))
        );
    }
}
#[test]
fn does_not_log_warning_when_only_new_property_is_defined() {
    for (canonical, legacy) in fixture::MAPPING_PAIRS {
        let source = OwnedPropertySource::new([(canonical.into(), "my-value".into())]);
        let mut warnings = Vec::new();
        PropertyFallbackRules::community_test_profile()
            .resolve(&source, canonical, &mut |w| warnings.push(w));
        let _ = legacy;
        assert!(warnings.is_empty());
    }
}

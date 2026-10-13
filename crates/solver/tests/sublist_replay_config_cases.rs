// Adapted pinned Community SubListSelectorFactoryTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

#[test]
fn replay_rejects_maximum_before_lookup() {
    let config = SubListSelectionConfig {
        maximum: Some(10),
        replay_ref: Some("someSelectorId".into()),
        ..Default::default()
    };
    let result = config.build_replaying(&SelectionRegistry::new());
    let error = result.err().expect("explicit bound must reject replay");
    assert!(matches!(error, SubListSelectionBuildError::ReplayOverrides));
    assert!(error.to_string().contains("has another property"));
}

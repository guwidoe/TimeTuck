use std::cell::Cell;
use timetuck::*;
#[test]
fn loader_precedence_fallback_and_live_package_are_actual_metadata() {
    let calls = Cell::new(0);
    let primary = VersionMetadata::new("3.7.9-rc-42", Some("abc1234".into())).unwrap();
    let fallback = VersionMetadata::new("0.9.1", Some("def5678".into())).unwrap();
    let r = VersionReporter::resolve(
        "selected",
        |name| {
            assert_eq!(name, "selected");
            calls.set(calls.get() + 1);
            Some(primary)
        },
        Some(fallback.clone()),
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(r.bare_version(), "3.7.9");
    assert_eq!(
        r.identify(),
        format!("{COMMUNITY_EDITION} v3.7.9 (abc1234)")
    );
    let r = VersionReporter::resolve("absent", |_| None, Some(fallback));
    assert_eq!(r.banner("custom"), "custom v0.9.1");
    assert_eq!(r.revision(), Some("def5678"));
    assert_eq!(
        VersionMetadata::package().version(),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(VersionReporter::package().revision(), None);
}
#[test]
fn invalid_metadata_and_non_suffix_release_labels_are_retained_or_rejected() {
    for version in ["", "1.2 3", "1.2\n3"] {
        assert!(VersionMetadata::new(version, None).is_err());
    }
    for revision in ["abcdef", "A123456", "xyz1234"] {
        assert!(VersionMetadata::new("1.0", Some(revision.into())).is_err());
    }
    for label in [
        "2-rc-x",
        "2-rc-",
        "2-rc-2-tail",
        "2-rc-2.1",
        "2-rc-\u{0661}",
    ] {
        assert_eq!(strip_release_candidate(Some(label)), Some(label));
    }
    for revision in ["abc1234".to_string(), "a".repeat(40)] {
        assert_eq!(
            VersionMetadata::new("1.0", Some(revision.clone()))
                .unwrap()
                .revision(),
            Some(revision.as_str())
        );
    }
}

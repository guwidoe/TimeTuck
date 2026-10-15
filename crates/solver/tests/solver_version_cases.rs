// Adapted pinned Community SolverVersionUtilsTest, Apache-2.0.
use timetuck::*;
fn missing() -> VersionReporter {
    VersionReporter::resolve("does-not-exist-git.properties", |_| None, None)
}
#[test]
fn strip_release_candidate_strips_trailing_rc_suffix() {
    for version in ["1.2.3-rc-2", "1.2.3-rc-10"] {
        assert_eq!(strip_release_candidate(Some(version)), Some("1.2.3"));
    }
}
#[test]
fn strip_release_candidate_leaves_non_rc_versions_unchanged() {
    for version in [
        "1.2.3",
        "1.2.3-beta-4",
        "999-SNAPSHOT",
        "1.2.3-RC-2",
        "1.2.3-rc.2",
        "1.2.3-rc",
    ] {
        assert_eq!(strip_release_candidate(Some(version)), Some(version));
    }
}
#[test]
fn strip_release_candidate_handles_null() {
    assert_eq!(strip_release_candidate(None), None);
}
#[test]
fn banner_falls_back_to_manifest_when_resource_missing() {
    assert_eq!(
        missing().banner("My Edition"),
        format!("My Edition {DEVELOPMENT_SNAPSHOT}")
    );
}
#[test]
fn banner_prefers_real_git_properties_over_absent_manifest_version() {
    // Actual native Cargo metadata replaces the source build-generated resource.
    let r = VersionReporter::resolve("cargo-package", |_| Some(VersionMetadata::package()), None);
    let banner = r.banner("My Edition");
    assert!(!banner.contains(DEVELOPMENT_SNAPSHOT));
    let version = banner.strip_prefix("My Edition v").unwrap();
    assert!(!version.is_empty() && !version.chars().any(char::is_whitespace));
}
#[test]
fn bare_version_prefers_real_git_properties_over_absent_manifest_version() {
    assert_ne!(
        VersionReporter::resolve("cargo-package", |_| Some(VersionMetadata::package()), None)
            .bare_version(),
        "SNAPSHOT"
    );
}
#[test]
fn community_banner_with_git_ref_with_ref() {
    assert_eq!(
        missing().community_banner(Some("a1b2c3d")),
        format!("{COMMUNITY_EDITION} {DEVELOPMENT_SNAPSHOT} (a1b2c3d)")
    );
}
#[test]
fn community_banner_with_git_ref_without_ref() {
    assert_eq!(
        missing().community_banner(None),
        format!("{COMMUNITY_EDITION} {DEVELOPMENT_SNAPSHOT}")
    );
}
#[test]
fn git_ref_of_core_properties_is_null_or_valid_sha() {
    if let Some(revision) = VersionReporter::package().revision() {
        assert!((7..=40).contains(&revision.len()));
        assert!(
            revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }
}
#[test]
fn identify_solver_version_with_git_ref_matches_community_pattern() {
    // Native product name replaces the Java product prefix; no Enterprise lookup.
    let s = VersionReporter::package().identify();
    assert!(
        !s.strip_prefix(&format!("{COMMUNITY_EDITION} "))
            .unwrap()
            .is_empty()
    );
}

// Adapted Community TimefoldSolverEnterpriseServiceTest#solverVersion,
// Apache-2.0. Native package diagnostics replace the JVM service utility.
#[test]
fn community_service_identifies_community_edition() {
    assert!(
        VersionReporter::package()
            .identify()
            .contains("Community Edition")
    );
}

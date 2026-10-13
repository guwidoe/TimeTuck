// Adapted pinned Community TEST-only display assertions, Apache-2.0.
// Source ENTERPRISE_NAME maps explicitly to a caller product label. The source
// module label "enterprise" remains ordinary supplied text; no runtime support.
use timetuck::*;
const PRODUCT: &str = "TimeTuck Solver Bundle";
const MISSING: &str = "does-not-exist-git.properties";
fn missing(resource: &str) -> Option<VersionMetadata> {
    assert_eq!(resource, MISSING);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(resource);
    assert_eq!(
        std::fs::read_to_string(path).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    None
}
fn banner(core: Option<&str>, extension: Option<&str>) -> String {
    let reporter = VersionReporter::resolve(MISSING, missing, None);
    let module = |name: &str, revision: Option<&str>| {
        VersionModuleProvenance::resolve(
            name,
            MISSING,
            missing,
            revision
                .map(|r| VersionMetadata::new(env!("CARGO_PKG_VERSION"), Some(r.into())).unwrap()),
        )
        .unwrap()
    };
    reporter
        .module_provenance_banner(
            PRODUCT,
            &[module("core", core), module("enterprise", extension)],
        )
        .unwrap()
}
#[test]
fn enterprise_banner_with_git_ref_both_refs() {
    assert_eq!(
        banner(Some("aaa1111"), Some("bbb2222")),
        format!("{PRODUCT} {DEVELOPMENT_SNAPSHOT} (core aaa1111, enterprise bbb2222)")
    );
}
#[test]
fn enterprise_banner_with_git_ref_only_core_ref() {
    assert_eq!(
        banner(Some("aaa1111"), None),
        format!("{PRODUCT} {DEVELOPMENT_SNAPSHOT} (core aaa1111)")
    );
}
#[test]
fn enterprise_banner_with_git_ref_only_enterprise_ref() {
    assert_eq!(
        banner(None, Some("bbb2222")),
        format!("{PRODUCT} {DEVELOPMENT_SNAPSHOT} (enterprise bbb2222)")
    );
}
#[test]
fn enterprise_banner_with_git_ref_no_refs() {
    assert_eq!(
        banner(None, None),
        format!("{PRODUCT} {DEVELOPMENT_SNAPSHOT}")
    );
}

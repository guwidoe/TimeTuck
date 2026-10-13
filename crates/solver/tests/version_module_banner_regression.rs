// Original formatter/resource controls; no additional source credit.
use timetuck::*;
#[test]
fn real_resources_unknown_metadata_and_ref_omission_follow_caller_order() {
    let path = std::env::temp_dir().join(format!(
        "timetuck-version-resource-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, r#"{"version":"3.4.5-rc-2","revision":"abcdef0"}"#).unwrap();
    let load = |resource: &str| {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(resource).unwrap()).unwrap();
        Some(
            VersionMetadata::new(
                v["version"].as_str().unwrap(),
                Some(v["revision"].as_str().unwrap().into()),
            )
            .unwrap(),
        )
    };
    let resource = path.to_str().unwrap();
    let reporter = VersionReporter::resolve(resource, load, None);
    let module = VersionModuleProvenance::resolve("plugin-api", resource, load, None).unwrap();
    let unknown = VersionModuleProvenance::resolve("unknown", "absent", |_| None, None).unwrap();
    let no_revision = VersionModuleProvenance::resolve(
        "native",
        "package",
        |_| Some(VersionMetadata::package()),
        None,
    )
    .unwrap();
    assert_eq!(
        reporter
            .module_provenance_banner("My Product", &[unknown, module.clone(), no_revision])
            .unwrap(),
        "My Product v3.4.5 (plugin-api abcdef0)"
    );
    assert_eq!(module.reporter().revision(), Some("abcdef0"));
    let reporter = VersionReporter::resolve("unknown", |_| None, None);
    assert_eq!(
        reporter
            .module_provenance_banner("Other Product", &[])
            .unwrap(),
        format!("Other Product {DEVELOPMENT_SNAPSHOT}")
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn invalid_names_refs_and_duplicate_modules_reject_without_forged_provenance() {
    let reporter = VersionReporter::package();
    for invalid in [
        "",
        " leading",
        "trailing ",
        "fake\nline",
        "fake(module)",
        "fake,module",
    ] {
        assert!(reporter.module_provenance_banner(invalid, &[]).is_err());
    }
    for invalid in ["", "two words", "bad,ref", "bad\nname"] {
        assert!(
            VersionModuleProvenance::resolve(
                invalid,
                "resource",
                |_| panic!("invalid name must reject before resource callback"),
                None
            )
            .is_err()
        );
    }
    for invalid in ["UNKNOWN", "abcdef", "abcdefg", "AABBCCD"] {
        assert!(VersionMetadata::new("1.2.3", Some(invalid.into())).is_err());
    }
    let module = VersionModuleProvenance::resolve(
        "core",
        "package",
        |_| Some(VersionMetadata::package()),
        None,
    )
    .unwrap();
    assert_eq!(
        reporter
            .module_provenance_banner("Bundle", &[module.clone(), module])
            .unwrap_err(),
        ModuleBannerError::DuplicateModule("core".into())
    );
}

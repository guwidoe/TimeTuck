// Apache-2.0; complete ConfigureMojoTest contracts adapted. See provenance.
#[path = "fixtures/platform_configuration_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn test_skip_by_parameter() {
    let mut c = client(vec![]);
    let mut o = options();
    o.deploy_requested = false;
    o.skip = true;
    c.configure(&o, &[], 0, None, None).unwrap();
    assert!(c.logs.iter().any(|e| {
        e.level == PlatformLogLevel::Info
            && e.message
                .contains("Timefold Platform configuration skipped")
    }));
}
#[test]
fn test_configure_successfully() {
    let mut c = client(vec![success()]);
    let mut b = Builder::default();
    let out = c
        .configure(
            &options(),
            &projects(),
            0,
            Some(&mut b),
            Some(&token("xxxx")),
        )
        .unwrap()
        .unwrap();
    requested(&c, 1);
    configured(&c);
    let p = read_output(&out);
    for (k, v) in [
        ("quarkus.container-image.group", "test"),
        ("quarkus.container-image.registry", "test.registry.com"),
        ("quarkus.container-image.push", "true"),
        ("image.native-suffix", ""),
    ] {
        assert_eq!(p.get(k).map(String::as_str), Some(v));
    }
}
#[test]
fn test_configure_successfully_when_platform_reports_namespaces() {
    let mut reply = account(
        &["registered-model:create"],
        Some(("namespaces", &["test"])),
    );
    let mut value: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    value["roles"] = serde_json::json!(["model-publisher"]);
    reply.body = serde_json::to_vec(&value).unwrap();
    let mut c = client(vec![reply]);
    let mut b = Builder::default();
    let out = c
        .configure(
            &options(),
            &projects(),
            0,
            Some(&mut b),
            Some(&token("namespaces")),
        )
        .unwrap()
        .unwrap();
    configured(&c);
    assert_eq!(read_output(&out)["quarkus.container-image.group"], "test");
}
#[test]
fn test_configure_successfully_with_trailing_slash_in_platform_url() {
    let mut c = client(vec![success()]);
    let mut b = Builder::default();
    let mut o = options();
    o.platform_url.push('/');
    c.configure(&o, &projects(), 0, Some(&mut b), Some(&token("xxxx")))
        .unwrap();
    requested(&c, 1);
    configured(&c);
}
#[test]
fn test_configure_fails_with_blank_platform_url() {
    let mut c = client(vec![]);
    let mut b = Builder::default();
    let mut o = options();
    o.platform_url = "///".into();
    assert_eq!(
        c.configure(&o, &projects(), 0, Some(&mut b), Some(&token("xxxx")))
            .unwrap_err()
            .0,
        "Platform Url is mandatory"
    );
    requested(&c, 0);
}
#[test]
fn test_configure_successfully_native_supported() {
    let mut c = client(vec![success()]);
    let mut b = Builder {
        format: NativeImageFormat::Native,
        ..Builder::default()
    };
    let out = c
        .configure(
            &options(),
            &projects(),
            0,
            Some(&mut b),
            Some(&token("xxxx")),
        )
        .unwrap()
        .unwrap();
    requested(&c, 1);
    configured(&c);
    let p = read_output(&out);
    for (k, v) in [
        ("quarkus.container-image.group", "test"),
        ("quarkus.container-image.registry", "test.registry.com"),
        ("quarkus.container-image.push", "true"),
    ] {
        assert_eq!(p[k], v);
    }
    assert!(!p.contains_key("image.native-suffix"));
}
#[test]
fn test_configure_not_authorized() {
    let mut c = client(vec![response(401, "")]);
    let mut b = Builder::default();
    assert_eq!(
        c.configure(
            &options(),
            &projects(),
            0,
            Some(&mut b),
            Some(&token("wrong"))
        )
        .unwrap_err()
        .0,
        "Platform authentication failed — please verify your PAT and tenant access, or contact support if the problem persists"
    );
    requested(&c, 1);
}
#[test]
fn test_configure_missing_access_token() {
    let mut c = client(vec![]);
    let mut b = Builder::default();
    missing(
        &c.configure(&options(), &projects(), 0, Some(&mut b), None)
            .unwrap_err()
            .0,
    );
    requested(&c, 0);
}
#[test]
fn test_configure_needs_an_access_token_even_on_a_dry_run() {
    let mut c = client(vec![]);
    let mut b = Builder::default();
    let mut o = options();
    o.dry_run = true;
    assert!(
        c.configure(&o, &projects(), 0, Some(&mut b), None)
            .unwrap_err()
            .0
            .contains("Personal Access Token for Timefold Platform is required")
    );
}
fn namespace_error(
    reply: PlatformResponse,
    namespace: Option<&str>,
    token_value: &str,
) -> (String, PlatformConfigurator<Transport>) {
    let mut c = client(vec![reply]);
    let mut b = Builder::default();
    let mut o = options();
    o.namespace = namespace.map(str::to_owned);
    let message = c
        .configure(&o, &projects(), 0, Some(&mut b), Some(&token(token_value)))
        .unwrap_err()
        .0;
    requested(&c, 1);
    (message, c)
}
#[test]
fn test_configure_not_authorized_for_namespace() {
    assert_eq!(
        namespace_error(success(), Some("company"), "xxxx").0,
        "The personal access token is not associated with the configured namespace company, but with test"
    );
}
#[test]
fn test_configure_not_authorized_for_namespace_when_token_has_no_namespace() {
    assert_eq!(
        namespace_error(
            account(&["registered-model:create"], Some(("accountIds", &[]))),
            Some("company"),
            "emptyaccountids"
        )
        .0,
        "The personal access token is not associated with any namespace, so this model cannot be deployed to the configured namespace company"
    );
}
#[test]
fn test_configure_fails_when_token_has_no_namespace() {
    let message = namespace_error(
        account(&["registered-model:create"], Some(("accountIds", &[]))),
        None,
        "emptyaccountids",
    )
    .0;
    assert!(message.contains("Unable to resolve the Timefold Platform namespace"));
    assert!(message.contains("not associated with any namespace"));
}
#[test]
fn test_configure_fails_when_platform_reports_no_namespace_field() {
    assert!(
        namespace_error(
            account(&["registered-model:create"], None),
            None,
            "nonamespaces"
        )
        .0
        .contains("not associated with any namespace")
    );
}
#[test]
fn test_configure_fails_when_namespace_is_ambiguous() {
    let message = namespace_error(
        account(
            &["registered-model:create"],
            Some(("accountIds", &["test", "company"])),
        ),
        None,
        "multipleaccounts",
    )
    .0;
    assert!(message.contains("associated with 2 namespaces (company, test)"));
    assert!(message.contains("-Dtimefold.namespace=<namespace>"));
}
#[test]
fn test_configure_uses_configured_namespace_when_several_are_available() {
    let mut c = client(vec![account(
        &["registered-model:create"],
        Some(("accountIds", &["test", "company"])),
    )]);
    let mut b = Builder::default();
    let mut o = options();
    o.namespace = Some("company".into());
    let out = c
        .configure(
            &o,
            &projects(),
            0,
            Some(&mut b),
            Some(&token("multipleaccounts")),
        )
        .unwrap()
        .unwrap();
    requested(&c, 1);
    configured(&c);
    assert_eq!(
        read_output(&out)["quarkus.container-image.group"],
        "company"
    );
}
#[test]
fn test_configure_wrong_scopes() {
    assert_eq!(
        namespace_error(
            account(&[], Some(("accountIds", &["test"]))),
            None,
            "noaccess"
        )
        .0,
        "No access to deploy model on Timefold Platform"
    );
}
#[test]
fn test_configure_fails_when_service_parent_is_missing() {
    let mut c = client(vec![]);
    let mut b = Builder::default();
    let mut p = projects();
    p[0].parent = None;
    assert!(
        c.configure(&options(), &p, 0, Some(&mut b), Some(&token("xxxx")))
            .unwrap_err()
            .0
            .contains("does not inherit from ai.timefold.solver:timefold-solver-service-parent")
    );
    requested(&c, 0);
}
#[test]
fn test_configure_fails_when_service_parent_is_not_an_ancestor() {
    let mut c = client(vec![]);
    let mut b = Builder::default();
    let mut p = projects();
    p[1].group = "com.example".into();
    p[1].artifact = "some-other-parent".into();
    assert!(
        c.configure(&options(), &p, 0, Some(&mut b), Some(&token("xxxx")))
            .unwrap_err()
            .0
            .contains("does not inherit from ai.timefold.solver:timefold-solver-service-parent")
    );
}
#[test]
fn test_configure_accepts_service_parent_as_indirect_ancestor() {
    let mut c = client(vec![success()]);
    let mut b = Builder::default();
    let mut p = projects();
    p.push(ConfigureProject {
        group: "com.example".into(),
        artifact: "company-parent".into(),
        parent: Some(1),
    });
    p[0].parent = Some(2);
    c.configure(&options(), &p, 0, Some(&mut b), Some(&token("xxxx")))
        .unwrap();
    configured(&c);
}
#[test]
fn test_configure_skips_enterprise_check_when_deploy_not_requested() {
    let mut c = client(vec![]);
    let mut o = options();
    o.deploy_requested = false;
    c.configure(&o, &[], 0, None, None).unwrap();
    assert!(c.logs.is_empty());
    requested(&c, 0);
}

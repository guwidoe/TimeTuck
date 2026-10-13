#[path = "fixtures/platform_deployment_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn parts(mut bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut result = vec![];
    while !bytes.is_empty() {
        let size = u64::from_be_bytes(bytes[..8].try_into().unwrap()) as usize;
        bytes = &bytes[8..];
        result.push(bytes[..size].to_vec());
        bytes = &bytes[size..];
    }
    result
}
#[test]
fn actual_upload_and_override_preserve_binary_parts() {
    let archive = vec![0, 255, 13, 10, 0, 42];
    let executable = vec![255, 0, 9, 13, 10, 0];
    let p = DeploymentPackage::new(
        br#"{"id":"other-model","custom":"metadata"}"#.to_vec(),
        br#"{"version":"2.4.7"}"#.to_vec(),
        archive.clone(),
        Some(executable.clone()),
    )
    .unwrap();
    let mut o = options("a/b ?");
    o.descriptor_only = false;
    o.package_requested = true;
    o.model_type = Some(PlatformModelType::Private);
    o.tenants = vec!["different".into()];
    let mut c = client(vec![
        conflict("TFP-14005", "version conflict"),
        response(200, "{}"),
    ]);
    assert_eq!(
        c.deploy(
            Some(&p),
            &o,
            Some(&AccessTokenSecret::new("dummy-other-token"))
        )
        .unwrap(),
        DeploymentOutcome::Updated
    );
    let requests = &c.transport().requests;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].path, "/api/platform/v1/models/a%2Fb%20%3F");
    assert!(requests[1].query.is_empty());
    assert_eq!(requests[0].body, requests[1].body);
    assert_eq!(
        requests[0].authorization.expose(),
        "Bearer dummy-other-token"
    );
    assert!(!format!("{:?}", requests[0]).contains("dummy-other-token"));
    let chunks = parts(&requests[0].body);
    assert_eq!(chunks[1], archive);
    assert_eq!(chunks[2], executable);
    let metadata: serde_json::Value = serde_json::from_slice(&chunks[0]).unwrap();
    assert_eq!(metadata["descriptor"]["custom"], "metadata");
    assert_eq!(metadata["build"]["version"], "2.4.7");
    assert_eq!(metadata["tenants"], serde_json::json!(["different"]));
}
#[test]
fn descriptor_fixture_bytes_uploaded_without_substitution() {
    let mut c = client(vec![response(200, "{}")]);
    c.deploy(Some(&package()), &options("model-key"), Some(&token()))
        .unwrap();
    let chunks = parts(&c.transport().requests[0].body);
    assert_eq!(
        chunks[1],
        include_str!("fixtures/platform_deployment/model-descriptor.hex")
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    );
    assert!(chunks[2].is_empty());
    let m: serde_json::Value = serde_json::from_slice(&chunks[0]).unwrap();
    assert_eq!(
        m["descriptor"],
        serde_json::from_slice::<serde_json::Value>(include_bytes!(
            "fixtures/platform_deployment/model-descriptor.json"
        ))
        .unwrap()
    );
    assert_eq!(m["handleSubscription"], true);
}
#[test]
fn validation_and_skip_dry_run_precede_transport() {
    for desc in [b"[]".as_slice(), br#"{"id":""}"#, b"not json"] {
        assert!(
            DeploymentPackage::new(desc.to_vec(), br#"{"version":"1"}"#.to_vec(), vec![1], None)
                .is_err()
        );
    }
    assert!(
        DeploymentPackage::new(br#"{"id":"ok"}"#.to_vec(), b"{}".to_vec(), vec![1], None).is_err()
    );
    let mut c = client(vec![]);
    let mut o = options("");
    assert!(c.deploy(Some(&package()), &o, Some(&token())).is_err());
    o.skip = true;
    assert_eq!(
        c.deploy(None, &o, None).unwrap(),
        DeploymentOutcome::Skipped
    );
    o.skip = false;
    o.key = "valid".into();
    o.dry_run = true;
    assert_eq!(
        c.deploy(Some(&package()), &o, None).unwrap(),
        DeploymentOutcome::DryRun
    );
    assert!(c.transport().requests.is_empty());
}
#[test]
fn disabled_overwrite_never_patches_and_malformed_success_is_rejected() {
    let mut c = client(vec![
        conflict("TFP-14001", "real conflict"),
        response(200, "not json"),
    ]);
    let mut o = options("arbitrary");
    o.overwrite = false;
    assert!(
        c.deploy(Some(&package()), &o, Some(&token()))
            .unwrap_err()
            .0
            .contains("real conflict")
    );
    patch(&c, "arbitrary", 0);
    assert!(
        c.deploy(Some(&package()), &o, Some(&token()))
            .unwrap_err()
            .0
            .contains("Invalid platform success response")
    );
    assert_eq!(c.transport().requests.len(), 2);
}
#[test]
fn permission_validation_and_transport_failure_execute() {
    let mut c = client(vec![response(
        200,
        r#"{"user":"x","scopes":[1],"tenants":[],"accountIds":[]}"#,
    )]);
    assert!(
        c.permissions(None, Some(&token()))
            .unwrap_err()
            .0
            .contains("requires strings")
    );
    assert!(c.logs.is_empty());
    let mut calls = 0;
    let mut c = PlatformDeploymentClient::new(|r: PlatformRequest| {
        calls += 1;
        assert_eq!(r.method, PlatformMethod::Delete);
        Err("offline".into())
    });
    assert_eq!(
        c.undeploy("other", false, Some(&token())).unwrap_err().0,
        "Platform transport failed: offline"
    );
    drop(c);
    assert_eq!(calls, 1);
}

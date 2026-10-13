#[path = "fixtures/platform_configuration_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn actual_builder_consumes_current_configuration_and_owned_files() {
    for (namespace, registry, dry) in [
        ("other", "registry.example", false),
        ("dry", "localhost:4000", true),
    ] {
        let body = serde_json::json!({"scopes":["registered-model:create"],"namespaces":[namespace],"config":{"containerRegistry":registry},"unknown":{"nested":true}});
        let mut c = client(vec![response(200, &body.to_string())]);
        let mut o = options();
        o.dry_run = dry;
        o.platform_url = "http://native.example///".into();
        let mut calls = 0;
        let mut builder = |plan: &NativeImagePlan| {
            calls += 1;
            assert_eq!(plan.namespace, namespace);
            assert_eq!(plan.registry, registry);
            assert_eq!(plan.push, !dry);
            Ok(NativeImageArtifact{format:NativeImageFormat::Native,bytes:serde_json::to_vec(&serde_json::json!({"runtime":{"namespace":plan.namespace,"registry":plan.registry,"push":plan.push},"computed":plan.namespace.len()+plan.registry.len()})).unwrap()})
        };
        let out = c
            .configure(
                &o,
                &projects(),
                0,
                Some(&mut builder),
                Some(&token("dummy-secret")),
            )
            .unwrap()
            .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(c.transport().requests[0].base_url, "http://native.example");
        assert_eq!(
            c.transport().requests[0].request.authorization.expose(),
            "Bearer dummy-secret"
        );
        assert!(!format!("{:?}", c.transport().requests[0]).contains("dummy-secret"));
        let p = read_output(&out);
        assert_eq!(p["quarkus.container-image.push"], (!dry).to_string());
        let parsed: serde_json::Value = serde_json::from_slice(&out.artifact.bytes).unwrap();
        assert_eq!(parsed["runtime"]["namespace"], namespace);
        assert_eq!(parsed["computed"], namespace.len() + registry.len());
    }
}
#[test]
fn ancestry_and_response_failures_silence_builder() {
    let mut c = client(vec![]);
    let mut p = projects();
    p[1].parent = Some(0);
    let mut calls = 0;
    let mut builder = |_: &NativeImagePlan| {
        calls += 1;
        Err("should not execute".into())
    };
    assert!(
        c.configure(&options(), &p, 0, Some(&mut builder), Some(&token("xxxx")))
            .unwrap_err()
            .0
            .contains("cycle")
    );
    requested(&c, 0);
    for body in [
        "not json",
        r#"{"scopes":[1]}"#,
        r#"{"scopes":["registered-model:create"],"namespaces":["n"]}"#,
    ] {
        let mut c = client(vec![response(200, body)]);
        assert!(
            c.configure(
                &options(),
                &projects(),
                0,
                Some(&mut builder),
                Some(&token("xxxx"))
            )
            .is_err()
        );
        assert!(c.logs.is_empty());
        requested(&c, 1);
    }
    assert_eq!(calls, 0);
}
#[test]
fn builder_errors_produce_no_success_and_can_retry() {
    let mut c = client(vec![success(), success(), success()]);
    let mut calls = 0;
    let mut builder = |plan: &NativeImagePlan| {
        calls += 1;
        if calls == 1 {
            return Err("compiler failed".into());
        }
        Ok(NativeImageArtifact {
            format: NativeImageFormat::Portable,
            bytes: if calls == 2 {
                vec![]
            } else {
                plan.namespace.as_bytes().to_vec()
            },
        })
    };
    assert!(
        c.configure(
            &options(),
            &projects(),
            0,
            Some(&mut builder),
            Some(&token("xxxx"))
        )
        .unwrap_err()
        .0
        .contains("compiler failed")
    );
    assert!(c.logs.is_empty());
    assert!(
        c.configure(
            &options(),
            &projects(),
            0,
            Some(&mut builder),
            Some(&token("xxxx"))
        )
        .unwrap_err()
        .0
        .contains("empty artifact")
    );
    assert!(c.logs.is_empty());
    assert!(
        c.configure(
            &options(),
            &projects(),
            0,
            Some(&mut builder),
            Some(&token("xxxx"))
        )
        .unwrap()
        .is_some()
    );
    configured(&c);
    assert_eq!(calls, 3);
}

#[test]
fn missing_builder_rejects_before_requests_even_when_skip_is_set() {
    for skip in [false, true] {
        let mut c = client(vec![]);
        let mut o = options();
        o.skip = skip;
        assert_eq!(
            c.configure(&o, &projects(), 0, None, Some(&token("xxxx")))
                .unwrap_err()
                .0,
            "Native deployment requires an executable NativeImageBuilder binding"
        );
        requested(&c, 0);
        assert!(c.logs.is_empty());
    }
}

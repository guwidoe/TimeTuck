// Apache-2.0; adapted full DeployModelMojoTest methods. See provenance.
#[path = "fixtures/platform_deployment_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn test_skip_by_parameter() {
    let mut c = client(vec![]);
    let mut o = options("model-key");
    o.skip = true;
    assert_eq!(
        c.deploy(Some(&package()), &o, Some(&token())).unwrap(),
        DeploymentOutcome::Skipped
    );
    log(
        &c,
        PlatformLogLevel::Info,
        "Model deployment skipped by configuration",
    );
}
#[test]
fn test_fails_without_access_token() {
    let mut c = client(vec![]);
    missing_token(
        &c.deploy(Some(&package()), &options("model-key"), None)
            .unwrap_err()
            .0,
    );
    post(&c, "model-key", 0);
}
#[test]
fn test_dry_run_does_not_need_an_access_token() {
    let mut c = client(vec![]);
    let mut o = options("model-key");
    o.dry_run = true;
    assert_eq!(
        c.deploy(Some(&package()), &o, None).unwrap(),
        DeploymentOutcome::DryRun
    );
    log(
        &c,
        PlatformLogLevel::Info,
        "DRY_RUN: Would perform POST on ",
    );
    post(&c, "model-key", 0);
}
#[test]
fn test_register_model() {
    let mut c = client(vec![response(200, "{}")]);
    c.deploy(Some(&package()), &options("model-key"), Some(&token()))
        .unwrap();
    post(&c, "model-key", 1);
    log(
        &c,
        PlatformLogLevel::Debug,
        "Type is not explicitly specified so it was computed based on tenants and is set to Private",
    );
    log(
        &c,
        PlatformLogLevel::Info,
        "has been successfully deployed into platform",
    );
}
fn patch_case(key: &str, code: &str, message: &str) {
    let mut c = client(vec![conflict(code, message), response(200, "{}")]);
    assert_eq!(
        c.deploy(Some(&package()), &options(key), Some(&token()))
            .unwrap(),
        DeploymentOutcome::Updated
    );
    post(&c, key, 1);
    patch(&c, key, 1);
    log(
        &c,
        PlatformLogLevel::Info,
        "has been successfully updated on platform",
    );
}
#[test]
fn test_register_model_with_patch() {
    patch_case(
        "existing",
        "TFP-14001",
        "Registered model conflicts with existing model (model_v1), unique model registration key is required",
    );
}
#[test]
fn test_register_model_with_patch_on_model_version_conflict() {
    patch_case(
        "existing-version",
        "TFP-14005",
        "Model build version '1.0.0' already matches the current latest release for registration key 'existing-version'; override the existing release instead of registering a duplicate.",
    );
}
fn unresolved(
    key: &str,
    code: &str,
    message: &str,
    reply: PlatformResponse,
) -> PlatformDeploymentClient<Server> {
    let mut c = client(vec![reply]);
    assert_eq!(
        c.deploy(Some(&package()), &options(key), Some(&token()))
            .unwrap_err()
            .0,
        format!(
            "Model deployment of timefold-test-model_v2-beta failed due to conflict ({code}) that cannot be resolved by updating the registration with key {key}: {message}"
        )
    );
    post(&c, key, 1);
    patch(&c, key, 0);
    c
}
#[test]
fn test_fail_without_patch_on_model_id_conflict() {
    let message = "Existing private model (model_v1), already exists for tenants";
    let c = unresolved(
        "existing-model-id",
        "TFP-14004",
        message,
        conflict("TFP-14004", message),
    );
    log(&c, PlatformLogLevel::Error, message);
}
#[test]
fn test_fail_without_patch_on_unidentified_conflict() {
    let message = "Registered model conflicts with existing model (model_v1)";
    let c = unresolved(
        "unknown-conflict",
        "TFP-99999",
        message,
        conflict("TFP-99999", message),
    );
    log(&c, PlatformLogLevel::Error, message);
}
#[test]
fn test_fail_on_update_reports_platform_error() {
    let message = "Model with registration key 'failing-update' was not found";
    let mut c = client(vec![
        conflict(
            "TFP-14001",
            "Registered model conflicts with existing model (model_v1), unique model registration key is required",
        ),
        response(
            404,
            &serde_json::json!({"id":"1","code":"TFP-99999","message":message}).to_string(),
        ),
    ]);
    assert_eq!(
        c.deploy(Some(&package()), &options("failing-update"), Some(&token()))
            .unwrap_err()
            .0,
        format!("Model deployment (override) failed with 404 status code: {message}")
    );
    patch(&c, "failing-update", 1);
    log(&c, PlatformLogLevel::Error, message);
}
#[test]
fn test_fail_without_patch_on_malformed_conflict_body() {
    let c = unresolved(
        "malformed-conflict",
        "TFP-99999",
        "<html>Conflict</html>",
        response(409, "<html>Conflict</html>"),
    );
    log(
        &c,
        PlatformLogLevel::Debug,
        "Unable to read error code from response body <html>Conflict</html>",
    );
    log(&c, PlatformLogLevel::Error, "<html>Conflict</html>");
}
#[test]
fn test_fail_without_patch_on_empty_conflict_body() {
    let c = unresolved(
        "empty-conflict",
        "TFP-99999",
        "no error message reported by the platform",
        response(409, ""),
    );
    assert!(c.logs.iter().all(|e| e.level != PlatformLogLevel::Error));
}
#[test]
fn test_fail_on_missing_model_descriptor() {
    let mut c = client(vec![]);
    assert_eq!(
        c.deploy(None, &options("model-key"), None).unwrap_err().0,
        "Model descriptor not found in target folder"
    );
}
#[test]
fn test_register_model_shared_type() {
    let mut c = client(vec![response(200, "{}")]);
    let mut o = options("model-key");
    o.tenants
        .push("007f172e-353f-440f-bf30-46321f6d6734".into());
    c.deploy(Some(&package()), &o, Some(&token())).unwrap();
    post(&c, "model-key", 1);
    log(
        &c,
        PlatformLogLevel::Debug,
        "Type is not explicitly specified so it was computed based on tenants and is set to Shared",
    );
    log(
        &c,
        PlatformLogLevel::Info,
        "has been successfully deployed into platform",
    );
}
#[test]
fn test_fail_on_incomplete_deploy() {
    let mut c = client(vec![]);
    let mut o = options("model-key");
    o.descriptor_only = false;
    assert_eq!(
        c.deploy(Some(&package()), &o, Some(&token()))
            .unwrap_err()
            .0,
        "'package' goal was not requested, deploy of Timefold model might not be complete, make sure to use 'clean package timefold:deploy' or set '-Dtimefold.model.deploy.descriptorOnly=true'"
    );
}

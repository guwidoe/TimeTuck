// Apache-2.0; adapted full UndeployModelMojoTest methods. See provenance.
#[path = "fixtures/platform_deployment_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
fn deleted(c: &PlatformDeploymentClient<Server>, key: &str) {
    assert_eq!(c.transport().requests.len(), 1);
    assert_eq!(c.transport().requests[0].method, PlatformMethod::Delete);
    assert_eq!(
        c.transport().requests[0].path,
        format!("/api/platform/v1/models/{key}")
    );
}
#[test]
fn test_skip_by_parameter() {
    let mut c = client(vec![]);
    assert_eq!(
        c.undeploy("model-key", true, Some(&token())).unwrap(),
        DeploymentOutcome::Skipped
    );
    log(
        &c,
        PlatformLogLevel::Info,
        "Model undeployment skipped by configuration",
    );
}
#[test]
fn test_fails_without_access_token() {
    let mut c = client(vec![]);
    missing_token(&c.undeploy("existing", false, None).unwrap_err().0);
    assert!(c.transport().requests.is_empty());
}
#[test]
fn test_undeploy() {
    let mut c = client(vec![response(200, "{}")]);
    assert_eq!(
        c.undeploy("existing", false, Some(&token())).unwrap(),
        DeploymentOutcome::Undeployed
    );
    deleted(&c, "existing");
    log(
        &c,
        PlatformLogLevel::Info,
        "has been successfully undeployed from platform",
    );
}
#[test]
fn test_undeploy_not_existing() {
    let msg = "Model with registration key 'notexisting' was not found";
    let mut c = client(vec![response(
        404,
        &serde_json::json!({"id":"1","code":"TFP-99999","message":msg}).to_string(),
    )]);
    assert_eq!(
        c.undeploy("notexisting", false, Some(&token()))
            .unwrap_err()
            .0,
        format!("Model undeploy failed with 404 status code: {msg}")
    );
    deleted(&c, "notexisting");
    log(&c, PlatformLogLevel::Error, msg);
}
#[test]
fn test_undeploy_failure_without_details() {
    let mut c = client(vec![response(500, "")]);
    assert_eq!(
        c.undeploy("nodetails", false, Some(&token()))
            .unwrap_err()
            .0,
        "Model undeploy failed with 500 status code: no error message reported by the platform"
    );
    deleted(&c, "nodetails");
    assert!(c.logs.iter().all(|e| e.level != PlatformLogLevel::Error));
}

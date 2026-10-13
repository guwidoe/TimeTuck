// Apache-2.0; adapted full PermissionsMojoTest methods. See provenance.
#[path = "fixtures/platform_deployment_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn test_reports_permissions() {
    let mut c=client(vec![response(200,&serde_json::json!({"user":"test@email.com","scopes":["registered-model:create","registered-model:update"],"tenants":[TENANT],"accountIds":["test"]}).to_string())]);
    c.permissions(Some(TENANT), Some(&token())).unwrap();
    assert_eq!(c.transport().requests.len(), 1);
    assert_eq!(c.transport().requests[0].path, "/api/platform/v1/aboutme");
    for s in [
        "test@email.com",
        "registered-model:create, registered-model:update",
        TENANT,
        "Selected tenant : 007f172e-353f-440f-bf30-46321f6d6733",
        "token can register/update models",
    ] {
        log(&c, PlatformLogLevel::Info, s);
    }
}
#[test]
fn test_warns_when_token_lacks_push_access() {
    let mut c = client(vec![response(
        200,
        r#"{"user":"test@email.com","scopes":[],"tenants":[],"accountIds":["test"]}"#,
    )]);
    c.permissions(Some(TENANT), Some(&AccessTokenSecret::new("noaccess")))
        .unwrap();
    assert_eq!(c.transport().requests.len(), 1);
    assert_eq!(c.transport().requests[0].path, "/api/platform/v1/aboutme");
    log(&c, PlatformLogLevel::Info, "Scopes : (none)");
    log(
        &c,
        PlatformLogLevel::Warn,
        "token CANNOT register/update models",
    );
}
#[test]
fn test_fails_when_not_authorized() {
    let mut c = client(vec![response(401, "")]);
    assert_eq!(
        c.permissions(Some(TENANT), Some(&AccessTokenSecret::new("wrong")))
            .unwrap_err()
            .0,
        "Platform authentication failed — please verify your PAT and tenant access, or contact support if the problem persists"
    );
    assert_eq!(c.transport().requests.len(), 1);
    assert_eq!(c.transport().requests[0].path, "/api/platform/v1/aboutme");
}
#[test]
fn test_fails_when_access_token_missing() {
    let mut c = client(vec![]);
    missing_token(&c.permissions(Some(TENANT), None).unwrap_err().0);
    assert!(c.transport().requests.is_empty());
}

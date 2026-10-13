#![allow(dead_code)] // Shared source fixture helpers serve separate adapted targets.
// Adapted from pinned Timefold Community Maven plugin TESTs/resources.
// Copyright Timefold contributors; Apache-2.0. See THIRD_PARTY_NOTICES.md.
use std::collections::VecDeque;
use timetuck::*;
pub const TENANT: &str = "007f172e-353f-440f-bf30-46321f6d6733";
pub fn package() -> DeploymentPackage {
    DeploymentPackage::new(
        include_bytes!("platform_deployment/model-descriptor.json").to_vec(),
        include_bytes!("platform_deployment/build-info.json").to_vec(),
        include_str!("platform_deployment/model-descriptor.hex")
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect(),
        None,
    )
    .unwrap()
}
pub fn options(key: &str) -> DeploymentOptions {
    DeploymentOptions {
        key: key.into(),
        tenants: vec![TENANT.into()],
        model_type: None,
        overwrite: true,
        descriptor_only: true,
        package_requested: false,
        handle_subscription: true,
        skip: false,
        dry_run: false,
    }
}
pub fn token() -> AccessTokenSecret {
    AccessTokenSecret::new("xxxx")
}
pub fn response(status: u16, body: &str) -> PlatformResponse {
    PlatformResponse {
        status,
        body: body.as_bytes().to_vec(),
    }
}
pub fn conflict(code: &str, message: &str) -> PlatformResponse {
    response(
        409,
        &serde_json::json!({"id":"1","code":code,"message":message}).to_string(),
    )
}
#[derive(Default)]
pub struct Server {
    pub requests: Vec<PlatformRequest>,
    pub responses: VecDeque<PlatformResponse>,
}
impl PlatformTransport for Server {
    fn execute(&mut self, r: PlatformRequest) -> Result<PlatformResponse, String> {
        self.requests.push(r);
        self.responses
            .pop_front()
            .ok_or("Unexpected request".into())
    }
}
pub fn client(responses: Vec<PlatformResponse>) -> PlatformDeploymentClient<Server> {
    PlatformDeploymentClient::new(Server {
        requests: vec![],
        responses: responses.into(),
    })
}
pub fn log(c: &PlatformDeploymentClient<Server>, level: PlatformLogLevel, part: &str) {
    assert!(
        c.logs
            .iter()
            .any(|e| e.level == level && e.message.contains(part)),
        "missing {part}: {:?}",
        c.logs
    );
}
pub fn missing_token(message: &str) {
    for part in [
        "Personal Access Token for Timefold Platform is required",
        "export TIMEFOLD_PAT=<your token>",
        "<id>timefold-platform</id>",
        "mvn --encrypt-password",
    ] {
        assert!(message.contains(part));
    }
}
pub fn post(c: &PlatformDeploymentClient<Server>, key: &str, count: usize) {
    assert_eq!(
        c.transport()
            .requests
            .iter()
            .filter(|r| r.method == PlatformMethod::Post
                && r.path == "/api/platform/v1/models"
                && r.query == vec![("registrationKey".into(), key.into())])
            .count(),
        count
    );
}
pub fn patch(c: &PlatformDeploymentClient<Server>, key: &str, count: usize) {
    assert_eq!(
        c.transport()
            .requests
            .iter()
            .filter(|r| r.method == PlatformMethod::Patch
                && r.path == format!("/api/platform/v1/models/{key}"))
            .count(),
        count
    );
}

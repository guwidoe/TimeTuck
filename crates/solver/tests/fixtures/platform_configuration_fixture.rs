#![allow(dead_code)] // Shared adapted fixture used across focused targets.
// Apache-2.0; Timefold Community ConfigureMojoTest TEST stubs/helpers adapted.
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
pub fn options() -> ConfigureOptions {
    ConfigureOptions {
        platform_url: "http://local.test".into(),
        namespace: None,
        deploy_requested: true,
        skip: false,
        dry_run: false,
    }
}
pub fn projects() -> Vec<ConfigureProject> {
    vec![
        ConfigureProject {
            group: "test".into(),
            artifact: "model".into(),
            parent: Some(1),
        },
        ConfigureProject {
            group: "ai.timefold.solver".into(),
            artifact: "timefold-solver-service-parent".into(),
            parent: None,
        },
    ]
}
pub fn token(value: &str) -> AccessTokenSecret {
    AccessTokenSecret::new(value)
}
pub fn response(status: u16, body: &str) -> PlatformResponse {
    PlatformResponse {
        status,
        body: body.as_bytes().to_vec(),
    }
}
pub fn account(scopes: &[&str], names: Option<(&str, &[&str])>) -> PlatformResponse {
    let mut v = serde_json::json!({"user":"test@email.com","scopes":scopes,"tenants":[],"config":{"containerRegistry":"test.registry.com"}});
    if let Some((key, names)) = names {
        v[key] = serde_json::json!(names);
    }
    response(200, &v.to_string())
}
pub fn success() -> PlatformResponse {
    account(
        &["registered-model:create"],
        Some(("accountIds", &["test"])),
    )
}
#[derive(Default)]
pub struct Transport {
    pub requests: Vec<ConfigureRequest>,
    pub replies: VecDeque<PlatformResponse>,
}
impl ConfigureTransport for Transport {
    fn execute(&mut self, r: ConfigureRequest) -> Result<PlatformResponse, String> {
        self.requests.push(r);
        self.replies.pop_front().ok_or("unexpected request".into())
    }
}
pub fn client(replies: Vec<PlatformResponse>) -> PlatformConfigurator<Transport> {
    PlatformConfigurator::new(Transport {
        requests: vec![],
        replies: replies.into(),
    })
}
pub struct Builder {
    pub format: NativeImageFormat,
    pub plans: Vec<NativeImagePlan>,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            format: NativeImageFormat::Portable,
            plans: vec![],
        }
    }
}
impl NativeImageBuilder for Builder {
    fn build(&mut self, p: &NativeImagePlan) -> Result<NativeImageArtifact, String> {
        self.plans.push(p.clone());
        Ok(NativeImageArtifact{format:self.format,bytes:serde_json::to_vec(&serde_json::json!({"namespace":p.namespace,"registry":p.registry,"push":p.push,"platformUrl":p.platform_url})).unwrap()})
    }
}
pub fn configured(c: &PlatformConfigurator<Transport>) {
    assert!(c.logs.iter().any(|e| {
        e.level == PlatformLogLevel::Info
            && e.message
                .contains("Configured Timefold Platform integration")
    }));
}
pub fn requested(c: &PlatformConfigurator<Transport>, count: usize) {
    assert_eq!(c.transport().requests.len(), count);
    for r in &c.transport().requests {
        assert_eq!(r.request.method, PlatformMethod::Get);
        assert_eq!(r.request.path, "/api/platform/v1/aboutme");
    }
}
pub fn missing(message: &str) {
    for p in [
        "Personal Access Token for Timefold Platform is required",
        "export TIMEFOLD_PAT=<your token>",
        "<id>timefold-platform</id>",
        "mvn --encrypt-password",
    ] {
        assert!(message.contains(p));
    }
}
static COUNTER: AtomicUsize = AtomicUsize::new(0);
pub fn temp() -> PathBuf {
    std::env::temp_dir().join(format!(
        "timetuck-config-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}
pub fn read_output(
    output: &GeneratedPlatformConfiguration,
) -> std::collections::BTreeMap<String, String> {
    let path = temp();
    output.write(&path).unwrap();
    assert!(path.join("timefold-build.properties").exists());
    let contents = std::fs::read_to_string(path.join("timefold-build.properties")).unwrap();
    assert_eq!(
        std::fs::read(path.join("native-image.package")).unwrap(),
        output.artifact.bytes
    );
    let result = contents
        .lines()
        .map(|line| {
            let (k, v) = line.split_once('=').unwrap();
            (k.to_owned(), v.to_owned())
        })
        .collect();
    std::fs::remove_dir_all(path).unwrap();
    result
}

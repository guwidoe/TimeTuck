//! Optional deployment client. Executed caller transport replaces HTTP/Maven;
//! descriptor metadata and deployment package bytes are supplied explicitly.
use crate::AccessTokenSecret;
use serde_json::{Value, json};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformMethod {
    Get,
    Post,
    Patch,
    Delete,
}
#[derive(Clone, PartialEq, Eq)]
pub struct PlatformRequest {
    pub method: PlatformMethod,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub authorization: AccessTokenSecret,
    pub content_type: String,
    pub body: Vec<u8>,
}
impl fmt::Debug for PlatformRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlatformRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
pub trait PlatformTransport {
    fn execute(&mut self, request: PlatformRequest) -> Result<PlatformResponse, String>;
}
impl<F: FnMut(PlatformRequest) -> Result<PlatformResponse, String>> PlatformTransport for F {
    fn execute(&mut self, request: PlatformRequest) -> Result<PlatformResponse, String> {
        self(request)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformLog {
    pub level: PlatformLogLevel,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformDeploymentError(pub String);
impl fmt::Display for PlatformDeploymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for PlatformDeploymentError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformModelType {
    Private,
    Shared,
}
#[derive(Clone, Debug)]
pub struct DeploymentPackage {
    model_id: String,
    descriptor: Vec<u8>,
    build_metadata: Vec<u8>,
    archive: Vec<u8>,
    executable: Option<Vec<u8>>,
}
impl DeploymentPackage {
    pub fn new(
        descriptor: Vec<u8>,
        build_metadata: Vec<u8>,
        archive: Vec<u8>,
        executable: Option<Vec<u8>>,
    ) -> Result<Self, PlatformDeploymentError> {
        let value: Value = serde_json::from_slice(&descriptor)
            .map_err(|e| PlatformDeploymentError(format!("Invalid model descriptor: {e}")))?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| PlatformDeploymentError("Model descriptor requires nonempty id".into()))?
            .to_owned();
        let build: Value = serde_json::from_slice(&build_metadata)
            .map_err(|e| PlatformDeploymentError(format!("Invalid build metadata: {e}")))?;
        if build
            .get("version")
            .is_none_or(|v| v.as_str().is_none_or(str::is_empty))
            || archive.is_empty()
            || executable.as_ref().is_some_and(Vec::is_empty)
        {
            return Err(PlatformDeploymentError(
                "Deployment package requires build version and nonempty artifact bytes".into(),
            ));
        }
        Ok(Self {
            model_id: id,
            descriptor,
            build_metadata,
            archive,
            executable,
        })
    }
}
#[derive(Clone, Debug)]
pub struct DeploymentOptions {
    pub key: String,
    pub tenants: Vec<String>,
    pub model_type: Option<PlatformModelType>,
    pub overwrite: bool,
    pub descriptor_only: bool,
    pub package_requested: bool,
    pub handle_subscription: bool,
    pub skip: bool,
    pub dry_run: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentOutcome {
    Skipped,
    DryRun,
    Registered,
    Updated,
    Undeployed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformPermissions {
    pub user: String,
    pub scopes: Vec<String>,
    pub tenants: Vec<String>,
    pub account_ids: Vec<String>,
}
pub struct PlatformDeploymentClient<T> {
    transport: T,
    pub logs: Vec<PlatformLog>,
}
const REQUIRED: &str = "Personal Access Token for Timefold Platform is required; export TIMEFOLD_PAT=<your token>; configure <id>timefold-platform</id>; use mvn --encrypt-password";
const AUTH_FAILED: &str = "Platform authentication failed — please verify your PAT and tenant access, or contact support if the problem persists";
impl<T: PlatformTransport> PlatformDeploymentClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            logs: Vec::new(),
        }
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    fn log(&mut self, level: PlatformLogLevel, message: impl Into<String>) {
        self.logs.push(PlatformLog {
            level,
            message: message.into(),
        });
    }
    fn token(
        token: Option<&AccessTokenSecret>,
    ) -> Result<AccessTokenSecret, PlatformDeploymentError> {
        token
            .filter(|s| !s.expose().trim().is_empty())
            .map(|s| AccessTokenSecret::new(format!("Bearer {}", s.expose())))
            .ok_or_else(|| PlatformDeploymentError(REQUIRED.into()))
    }
    fn send(
        &mut self,
        request: PlatformRequest,
    ) -> Result<PlatformResponse, PlatformDeploymentError> {
        self.transport
            .execute(request)
            .map_err(|e| PlatformDeploymentError(format!("Platform transport failed: {e}")))
    }
    fn success(response: &PlatformResponse) -> Result<(), PlatformDeploymentError> {
        if !serde_json::from_slice::<Value>(&response.body).is_ok_and(|v| v.is_object()) {
            return Err(PlatformDeploymentError(
                "Invalid platform success response: expected JSON object".into(),
            ));
        }
        Ok(())
    }
    fn response_error(&mut self, response: &PlatformResponse) -> (String, String) {
        if response.body.is_empty() {
            return (
                "TFP-99999".into(),
                "no error message reported by the platform".into(),
            );
        }
        let raw = String::from_utf8_lossy(&response.body).into_owned();
        let value = serde_json::from_slice::<Value>(&response.body).ok();
        let code = value
            .as_ref()
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str);
        if code.is_none() {
            self.log(
                PlatformLogLevel::Debug,
                format!("Unable to read error code from response body {raw}"),
            );
        }
        let message = value
            .as_ref()
            .and_then(|v| v.get("message"))
            .and_then(Value::as_str)
            .unwrap_or(&raw)
            .to_owned();
        self.log(PlatformLogLevel::Error, &message);
        (code.unwrap_or("TFP-99999").to_owned(), message)
    }
    pub fn deploy(
        &mut self,
        package: Option<&DeploymentPackage>,
        options: &DeploymentOptions,
        token: Option<&AccessTokenSecret>,
    ) -> Result<DeploymentOutcome, PlatformDeploymentError> {
        if options.skip {
            self.log(
                PlatformLogLevel::Info,
                "Model deployment skipped by configuration",
            );
            return Ok(DeploymentOutcome::Skipped);
        }
        let package = package.ok_or_else(|| {
            PlatformDeploymentError("Model descriptor not found in target folder".into())
        })?;
        if !options.descriptor_only && !options.package_requested {
            return Err(PlatformDeploymentError("'package' goal was not requested, deploy of Timefold model might not be complete, make sure to use 'clean package timefold:deploy' or set '-Dtimefold.model.deploy.descriptorOnly=true'".into()));
        }
        if !options.descriptor_only && package.executable.is_none() {
            return Err(PlatformDeploymentError(
                "Complete deployment requires executable artifact bytes".into(),
            ));
        }
        if options.key.trim().is_empty() || options.tenants.iter().any(|s| s.trim().is_empty()) {
            return Err(PlatformDeploymentError(
                "Registration key and tenant identifiers must be nonempty".into(),
            ));
        }
        let model_type = options.model_type.unwrap_or(if options.tenants.len() > 1 {
            PlatformModelType::Shared
        } else {
            PlatformModelType::Private
        });
        if options.model_type.is_none() {
            self.log(PlatformLogLevel::Debug, format!("Type is not explicitly specified so it was computed based on tenants and is set to {model_type:?}"));
        }
        // Native versioned envelope: actual JSON metadata plus binary parts, lengths
        // delimit arbitrary artifact bytes; no multipart HTTP or JAR ABI claim.
        let metadata = serde_json::to_vec(&json!({"schemaVersion":1,"modelId":package.model_id,"type":format!("{model_type:?}"),"tenants":options.tenants,"handleSubscription":options.handle_subscription,"descriptor":serde_json::from_slice::<Value>(&package.descriptor).map_err(|e| PlatformDeploymentError(e.to_string()))?,"build":serde_json::from_slice::<Value>(&package.build_metadata).map_err(|e| PlatformDeploymentError(e.to_string()))?})).map_err(|e| PlatformDeploymentError(e.to_string()))?;
        let mut body = Vec::new();
        for part in [
            &metadata[..],
            &package.archive[..],
            if options.descriptor_only {
                &[][..]
            } else {
                package.executable.as_deref().unwrap_or_default()
            },
        ] {
            body.extend_from_slice(&(part.len() as u64).to_be_bytes());
            body.extend_from_slice(part);
        }
        if options.dry_run {
            self.log(
                PlatformLogLevel::Info,
                "DRY_RUN: Would perform POST on /api/platform/v1/models",
            );
            return Ok(DeploymentOutcome::DryRun);
        }
        let mut request = PlatformRequest {
            method: PlatformMethod::Post,
            path: "/api/platform/v1/models".into(),
            query: vec![("registrationKey".into(), options.key.clone())],
            authorization: Self::token(token)?,
            content_type: "application/vnd.timetuck.deployment-v1".into(),
            body,
        };
        let response = self.send(request.clone())?;
        if (200..300).contains(&response.status) {
            Self::success(&response)?;
            self.log(
                PlatformLogLevel::Info,
                format!(
                    "Model {} has been successfully deployed into platform",
                    package.model_id
                ),
            );
            return Ok(DeploymentOutcome::Registered);
        }
        let (code, message) = self.response_error(&response);
        if response.status == 409 {
            if options.overwrite && matches!(code.as_str(), "TFP-14001" | "TFP-14005") {
                request.method = PlatformMethod::Patch;
                request.path = format!("/api/platform/v1/models/{}", encode_path(&options.key));
                request.query.clear();
                let response = self.send(request)?;
                if (200..300).contains(&response.status) {
                    Self::success(&response)?;
                    self.log(
                        PlatformLogLevel::Info,
                        format!(
                            "Model {} has been successfully updated on platform",
                            package.model_id
                        ),
                    );
                    return Ok(DeploymentOutcome::Updated);
                }
                let (_, message) = self.response_error(&response);
                return Err(PlatformDeploymentError(format!(
                    "Model deployment (override) failed with {} status code: {message}",
                    response.status
                )));
            }
            return Err(PlatformDeploymentError(format!(
                "Model deployment of {} failed due to conflict ({code}) that cannot be resolved by updating the registration with key {}: {message}",
                package.model_id, options.key
            )));
        }
        Err(PlatformDeploymentError(format!(
            "Model deployment failed with {} status code: {message}",
            response.status
        )))
    }
    pub fn undeploy(
        &mut self,
        key: &str,
        skip: bool,
        token: Option<&AccessTokenSecret>,
    ) -> Result<DeploymentOutcome, PlatformDeploymentError> {
        if skip {
            self.log(
                PlatformLogLevel::Info,
                "Model undeployment skipped by configuration",
            );
            return Ok(DeploymentOutcome::Skipped);
        }
        if key.trim().is_empty() {
            return Err(PlatformDeploymentError(
                "Registration key must be nonempty".into(),
            ));
        }
        let response = self.send(PlatformRequest {
            method: PlatformMethod::Delete,
            path: format!("/api/platform/v1/models/{}", encode_path(key)),
            query: vec![],
            authorization: Self::token(token)?,
            content_type: String::new(),
            body: vec![],
        })?;
        if (200..300).contains(&response.status) {
            Self::success(&response)?;
            self.log(
                PlatformLogLevel::Info,
                format!("Model {key} has been successfully undeployed from platform"),
            );
            return Ok(DeploymentOutcome::Undeployed);
        }
        let (_, message) = self.response_error(&response);
        Err(PlatformDeploymentError(format!(
            "Model undeploy failed with {} status code: {message}",
            response.status
        )))
    }
    pub fn permissions(
        &mut self,
        selected_tenant: Option<&str>,
        token: Option<&AccessTokenSecret>,
    ) -> Result<PlatformPermissions, PlatformDeploymentError> {
        let response = self.send(PlatformRequest {
            method: PlatformMethod::Get,
            path: "/api/platform/v1/aboutme".into(),
            query: vec![],
            authorization: Self::token(token)?,
            content_type: String::new(),
            body: vec![],
        })?;
        if response.status == 401 || response.status == 403 {
            return Err(PlatformDeploymentError(AUTH_FAILED.into()));
        }
        if !(200..300).contains(&response.status) {
            let (_, message) = self.response_error(&response);
            return Err(PlatformDeploymentError(format!(
                "Platform permissions failed with {} status code: {message}",
                response.status
            )));
        }
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|e| PlatformDeploymentError(format!("Invalid permissions response: {e}")))?;
        let strings = |name: &str| -> Result<Vec<String>, PlatformDeploymentError> {
            value
                .get(name)
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    PlatformDeploymentError(format!("Permissions require array {name}"))
                })?
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_owned).ok_or_else(|| {
                        PlatformDeploymentError(format!("Permissions {name} requires strings"))
                    })
                })
                .collect()
        };
        let result = PlatformPermissions {
            user: value
                .get("user")
                .and_then(Value::as_str)
                .ok_or_else(|| PlatformDeploymentError("Permissions require user".into()))?
                .into(),
            scopes: strings("scopes")?,
            tenants: strings("tenants")?,
            account_ids: strings("accountIds")?,
        };
        self.log(PlatformLogLevel::Info, &result.user);
        self.log(
            PlatformLogLevel::Info,
            format!(
                "Scopes : {}",
                if result.scopes.is_empty() {
                    "(none)".into()
                } else {
                    result.scopes.join(", ")
                }
            ),
        );
        self.log(PlatformLogLevel::Info, result.tenants.join(", "));
        if let Some(tenant) = selected_tenant {
            self.log(
                PlatformLogLevel::Info,
                format!("Selected tenant : {tenant}"),
            );
        }
        let push = ["registered-model:create", "registered-model:update"]
            .iter()
            .all(|s| result.scopes.iter().any(|v| v == s));
        self.log(
            if push {
                PlatformLogLevel::Info
            } else {
                PlatformLogLevel::Warn
            },
            if push {
                "token can register/update models"
            } else {
                "token CANNOT register/update models"
            },
        );
        Ok(result)
    }
}
fn encode_path(value: &str) -> String {
    let mut result = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            result.push(char::from(b));
        } else {
            result.push_str(&format!("%{b:02X}"));
        }
    }
    result
}

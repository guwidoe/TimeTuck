//! Executed native image-build configuration. No Enterprise/profile discovery.
use crate::{
    AccessTokenSecret, PlatformLog, PlatformLogLevel, PlatformMethod, PlatformRequest,
    PlatformResponse,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::Path,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureRequest {
    pub base_url: String,
    pub request: PlatformRequest,
}
pub trait ConfigureTransport {
    fn execute(&mut self, request: ConfigureRequest) -> Result<PlatformResponse, String>;
}
impl<F: FnMut(ConfigureRequest) -> Result<PlatformResponse, String>> ConfigureTransport for F {
    fn execute(&mut self, r: ConfigureRequest) -> Result<PlatformResponse, String> {
        self(r)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureProject {
    pub group: String,
    pub artifact: String,
    pub parent: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct ConfigureOptions {
    pub platform_url: String,
    pub namespace: Option<String>,
    pub deploy_requested: bool,
    pub skip: bool,
    pub dry_run: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeImageFormat {
    Portable,
    Native,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeImageArtifact {
    pub format: NativeImageFormat,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeImagePlan {
    pub namespace: String,
    pub registry: String,
    pub push: bool,
    pub platform_url: String,
}
pub trait NativeImageBuilder {
    fn build(&mut self, plan: &NativeImagePlan) -> Result<NativeImageArtifact, String>;
}
impl<F: FnMut(&NativeImagePlan) -> Result<NativeImageArtifact, String>> NativeImageBuilder for F {
    fn build(&mut self, p: &NativeImagePlan) -> Result<NativeImageArtifact, String> {
        self(p)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedPlatformConfiguration {
    pub plan: NativeImagePlan,
    pub artifact: NativeImageArtifact,
    pub properties: BTreeMap<String, String>,
}
impl GeneratedPlatformConfiguration {
    /// Writes actual owned output to a caller-selected local directory. No deployment.
    pub fn write(&self, directory: &Path) -> Result<(), PlatformConfigurationError> {
        fs::create_dir_all(directory).map_err(|e| PlatformConfigurationError(e.to_string()))?;
        let mut contents = String::new();
        for (k, v) in &self.properties {
            contents.push_str(k);
            contents.push('=');
            contents.push_str(&escape_property(v));
            contents.push('\n');
        }
        fs::write(directory.join("timefold-build.properties"), contents)
            .map_err(|e| PlatformConfigurationError(e.to_string()))?;
        fs::write(directory.join("native-image.package"), &self.artifact.bytes)
            .map_err(|e| PlatformConfigurationError(e.to_string()))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformConfigurationError(pub String);
impl fmt::Display for PlatformConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for PlatformConfigurationError {}
pub struct PlatformConfigurator<T> {
    transport: T,
    pub logs: Vec<PlatformLog>,
}
impl<T: ConfigureTransport> PlatformConfigurator<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            logs: vec![],
        }
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    fn log(&mut self, message: &str) {
        self.logs.push(PlatformLog {
            level: PlatformLogLevel::Info,
            message: message.into(),
        });
    }
    pub fn configure(
        &mut self,
        options: &ConfigureOptions,
        projects: &[ConfigureProject],
        model: usize,
        builder: Option<&mut dyn NativeImageBuilder>,
        token: Option<&AccessTokenSecret>,
    ) -> Result<Option<GeneratedPlatformConfiguration>, PlatformConfigurationError> {
        if options.deploy_requested {
            let mut at = projects
                .get(model)
                .ok_or_else(|| PlatformConfigurationError("Invalid model project index".into()))?
                .parent;
            let mut seen = BTreeSet::new();
            let mut found = false;
            while let Some(i) = at {
                if !seen.insert(i) {
                    return Err(PlatformConfigurationError(
                        "Project ancestry contains a cycle".into(),
                    ));
                }
                let p = projects.get(i).ok_or_else(|| {
                    PlatformConfigurationError("Invalid project ancestor index".into())
                })?;
                if p.group == "ai.timefold.solver" && p.artifact == "timefold-solver-service-parent"
                {
                    found = true;
                }
                at = p.parent;
            }
            if !found {
                return Err(PlatformConfigurationError(
                    "Model does not inherit from ai.timefold.solver:timefold-solver-service-parent"
                        .into(),
                ));
            }
            if builder.is_none() {
                return Err(PlatformConfigurationError(
                    "Native deployment requires an executable NativeImageBuilder binding".into(),
                ));
            }
        }
        if options.skip {
            self.log("Timefold Platform configuration skipped");
            return Ok(None);
        }
        if !options.deploy_requested {
            return Ok(None);
        }
        let url = options.platform_url.trim().trim_end_matches('/');
        if url.is_empty() {
            return Err(PlatformConfigurationError(
                "Platform Url is mandatory".into(),
            ));
        }
        let token=token.filter(|s|!s.expose().trim().is_empty()).ok_or_else(||PlatformConfigurationError("Personal Access Token for Timefold Platform is required; export TIMEFOLD_PAT=<your token>; configure <id>timefold-platform</id>; use mvn --encrypt-password".into()))?;
        let response = self
            .transport
            .execute(ConfigureRequest {
                base_url: url.into(),
                request: PlatformRequest {
                    method: PlatformMethod::Get,
                    path: "/api/platform/v1/aboutme".into(),
                    query: vec![],
                    authorization: AccessTokenSecret::new(format!("Bearer {}", token.expose())),
                    content_type: String::new(),
                    body: vec![],
                },
            })
            .map_err(|e| {
                PlatformConfigurationError(format!("Platform configuration transport failed: {e}"))
            })?;
        if response.status == 401 || response.status == 403 {
            return Err(PlatformConfigurationError("Platform authentication failed — please verify your PAT and tenant access, or contact support if the problem persists".into()));
        }
        if !(200..300).contains(&response.status) {
            return Err(PlatformConfigurationError(format!(
                "Platform configuration failed with {} status code",
                response.status
            )));
        }
        let value: Value = serde_json::from_slice(&response.body).map_err(|e| {
            PlatformConfigurationError(format!("Invalid platform configuration response: {e}"))
        })?;
        let scopes = strings(value.get("scopes"), "scopes")?;
        if !scopes.iter().any(|v| v == "registered-model:create") {
            return Err(PlatformConfigurationError(
                "No access to deploy model on Timefold Platform".into(),
            ));
        }
        let namespaces = strings(
            value.get("namespaces").or_else(|| value.get("accountIds")),
            "namespaces",
        )?;
        let namespace = if let Some(configured) = &options.namespace {
            if !namespaces.contains(configured) {
                return Err(PlatformConfigurationError(if namespaces.is_empty() {
                    format!(
                        "The personal access token is not associated with any namespace, so this model cannot be deployed to the configured namespace {configured}"
                    )
                } else {
                    format!(
                        "The personal access token is not associated with the configured namespace {configured}, but with {}",
                        namespaces.join(", ")
                    )
                }));
            }
            configured.clone()
        } else {
            match namespaces.len(){1=>namespaces[0].clone(),0=>return Err(PlatformConfigurationError("Unable to resolve the Timefold Platform namespace: personal access token is not associated with any namespace".into())),n=>return Err(PlatformConfigurationError(format!("Unable to resolve the Timefold Platform namespace: token associated with {n} namespaces ({}); configure -Dtimefold.namespace=<namespace>",namespaces.join(", "))))}
        };
        let registry = value
            .get("config")
            .and_then(|v| v.get("containerRegistry"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                PlatformConfigurationError(
                    "Platform configuration requires containerRegistry".into(),
                )
            })?
            .to_owned();
        let plan = NativeImagePlan {
            namespace,
            registry,
            push: !options.dry_run,
            platform_url: url.into(),
        };
        let artifact = builder
            .ok_or_else(|| PlatformConfigurationError("Native image builder missing".into()))?
            .build(&plan)
            .map_err(|e| PlatformConfigurationError(format!("Native image build failed: {e}")))?;
        if artifact.bytes.is_empty() {
            return Err(PlatformConfigurationError(
                "Native image builder returned empty artifact".into(),
            ));
        }
        let mut properties = BTreeMap::from([
            (
                "quarkus.container-image.group".into(),
                plan.namespace.clone(),
            ),
            (
                "quarkus.container-image.registry".into(),
                plan.registry.clone(),
            ),
            ("quarkus.container-image.push".into(), plan.push.to_string()),
        ]);
        if artifact.format == NativeImageFormat::Portable {
            properties.insert("image.native-suffix".into(), String::new());
        }
        let result = GeneratedPlatformConfiguration {
            plan,
            artifact,
            properties,
        };
        self.log("Configured Timefold Platform integration");
        Ok(Some(result))
    }
}
fn strings(value: Option<&Value>, name: &str) -> Result<Vec<String>, PlatformConfigurationError> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    let array = value.as_array().ok_or_else(|| {
        PlatformConfigurationError(format!("Platform {name} requires string array"))
    })?;
    let mut set = BTreeSet::new();
    for item in array {
        let s = item
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                PlatformConfigurationError(format!("Platform {name} requires nonempty strings"))
            })?;
        set.insert(s.to_owned());
    }
    Ok(set.into_iter().collect())
}
fn escape_property(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

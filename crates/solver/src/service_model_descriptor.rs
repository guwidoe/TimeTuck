//! Owned service model metadata; native validation and route consumption only.
use crate::EmployeeRouteDispatcher;
use std::fmt;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceDescriptorErrorKind {
    ModelId,
    ApiVersion,
    ResourcePath,
    ResourceMismatch,
    VersionMismatch,
    Dispatcher,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDescriptorError {
    pub kind: ServiceDescriptorErrorKind,
    pub message: String,
}
impl fmt::Display for ServiceDescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ServiceDescriptorError {}
fn error(kind: ServiceDescriptorErrorKind, message: impl Into<String>) -> ServiceDescriptorError {
    ServiceDescriptorError {
        kind,
        message: message.into(),
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ServiceModelId {
    value: String,
    model: String,
    version: Option<String>,
}
impl ServiceModelId {
    /// Native policy: <=255 ASCII bytes; optional single model/version separator.
    /// The TESTs establish the listed examples, not complete Java grammar parity.
    pub fn parse(input: Option<&str>) -> Result<Self, ServiceDescriptorError> {
        let value = input.filter(|s| !s.is_empty()).ok_or_else(|| {
            error(
                ServiceDescriptorErrorKind::ModelId,
                "Model name cannot be null or empty.",
            )
        })?;
        if value.len() > 255 {
            return Err(error(
                ServiceDescriptorErrorKind::ModelId,
                "Model name and version is too long.",
            ));
        }
        let underscores = value.bytes().filter(|&b| b == b'_').count();
        if underscores > 1 {
            return Err(error(
                ServiceDescriptorErrorKind::ModelId,
                "Model name cannot contain underscore (_).",
            ));
        }
        if value
            .bytes()
            .any(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'-' | b'_'))
        {
            return Err(error(
                ServiceDescriptorErrorKind::ModelId,
                "Model name can only contain letters (a-z, A-Z) and numbers (0-9), and hyphens (-).",
            ));
        }
        let (model, version) = value
            .split_once('_')
            .map_or((value, None), |(model, version)| (model, Some(version)));
        if model.is_empty() || version == Some("") {
            return Err(error(
                ServiceDescriptorErrorKind::ModelId,
                "Native model and optional version segments must be nonempty.",
            ));
        }
        Ok(Self {
            value: value.into(),
            model: model.into(),
            version: version.map(str::to_owned),
        })
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ServiceApiVersion(String);
impl ServiceApiVersion {
    /// Nonblank ASCII path segment, retained without implicit trimming.
    pub fn parse(input: Option<&str>) -> Result<Self, ServiceDescriptorError> {
        let value = input.filter(|v| !v.trim().is_empty()).ok_or_else(|| {
            error(
                ServiceDescriptorErrorKind::ApiVersion,
                "timefold.model.api-version must not be null, empty or blank.",
            )
        })?;
        if !safe_segment(value) {
            return Err(error(
                ServiceDescriptorErrorKind::ApiVersion,
                "timefold.model.api-version must be a native safe ASCII path segment.",
            ));
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ServiceResourceType(String);
impl ServiceResourceType {
    pub fn new(value: &str) -> Result<Self, ServiceDescriptorError> {
        if !safe_segment(value) {
            return Err(error(
                ServiceDescriptorErrorKind::ResourcePath,
                "Native resource must be a nonempty safe ASCII path segment.",
            ));
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceResourcePath {
    version: Option<ServiceApiVersion>,
    resource: ServiceResourceType,
}
impl ServiceResourcePath {
    /// Explicit native profile: optional v+digits prefix and one resource segment.
    pub fn parse(path: &str) -> Result<Self, ServiceDescriptorError> {
        if path.is_empty() {
            return Err(error(
                ServiceDescriptorErrorKind::ResourcePath,
                "Could not derive model resource type: ModelRest @Path value is empty.",
            ));
        }
        let mut segments = path.split('/').filter(|s| !s.is_empty());
        let first=segments.next().ok_or_else(||error(ServiceDescriptorErrorKind::ResourcePath,"Could not derive model resource type: ModelRest @Path does not contain path segments."))?;
        let versioned = first
            .strip_prefix('v')
            .is_some_and(|tail| !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()));
        let (version, resource) = if versioned {
            let resource=segments.next().ok_or_else(||error(ServiceDescriptorErrorKind::ResourcePath,"Could not derive model resource type: ModelRest @Path only contains API version but no resource segment."))?;
            (Some(ServiceApiVersion::parse(Some(first))?), resource)
        } else {
            (None, first)
        };
        if segments.next().is_some() {
            return Err(error(
                ServiceDescriptorErrorKind::ResourcePath,
                "Native model resource path must have one resource segment after its optional API version.",
            ));
        }
        Ok(Self {
            version,
            resource: ServiceResourceType::new(resource)?,
        })
    }
    pub fn version(&self) -> Option<&ServiceApiVersion> {
        self.version.as_ref()
    }
    pub fn resource(&self) -> &ServiceResourceType {
        &self.resource
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceModelDescriptor {
    model_id: ServiceModelId,
    api_version: ServiceApiVersion,
    path: ServiceResourcePath,
}
impl ServiceModelDescriptor {
    pub fn new(
        model_id: Option<&str>,
        api_version: Option<&str>,
        resource_path: &str,
    ) -> Result<Self, ServiceDescriptorError> {
        let model_id = ServiceModelId::parse(model_id)?;
        let api_version = ServiceApiVersion::parse(api_version)?;
        let path = ServiceResourcePath::parse(resource_path)?;
        if path.version().is_some_and(|v| v != &api_version) {
            return Err(error(
                ServiceDescriptorErrorKind::VersionMismatch,
                "Native resource path API version does not match the validated descriptor.",
            ));
        }
        Ok(Self {
            model_id,
            api_version,
            path,
        })
    }
    pub fn model_id(&self) -> &ServiceModelId {
        &self.model_id
    }
    pub fn api_version(&self) -> &ServiceApiVersion {
        &self.api_version
    }
    pub fn resource_type(&self) -> &ServiceResourceType {
        self.path.resource()
    }
    /// The existing employee backend implements schedules only. Validation occurs
    /// before any command can be queued or solver callback invoked.
    pub fn employee_dispatcher(
        &self,
        capacity: usize,
    ) -> Result<EmployeeRouteDispatcher, ServiceDescriptorError> {
        if self.resource_type().as_str() != "schedules" {
            return Err(error(
                ServiceDescriptorErrorKind::ResourceMismatch,
                format!(
                    "Employee routes require resource schedules, got {}.",
                    self.resource_type().as_str()
                ),
            ));
        }
        EmployeeRouteDispatcher::new(self.api_version.as_str(), capacity)
            .map_err(|e| error(ServiceDescriptorErrorKind::Dispatcher, e))
    }
}
fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && value != "."
        && value != ".."
}

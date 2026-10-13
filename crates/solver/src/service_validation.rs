//! Owned service validation records, legacy wire compatibility and issue summaries.
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}
impl ValidationSeverity {
    fn wire(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => "WARNING",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceValidationStatus {
    Ok,
    Warnings,
    Errors,
    NotSupported,
}
impl ServiceValidationStatus {
    fn wire(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warnings => "WARNINGS",
            Self::Errors => "ERRORS",
            Self::NotSupported => "VALIDATION_NOT_SUPPORTED",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceValidationError(pub String);
impl fmt::Display for ServiceValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ServiceValidationError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceValidationIssue {
    code: String,
    severity: ValidationSeverity,
    fields: Vec<(String, Value)>,
    legacy_message: Option<String>,
}
impl ServiceValidationIssue {
    /// Payload keys retain caller order; duplicate or protocol-reserved names fail
    /// before serialization. Values are real owned JSON, never reflective fields.
    pub fn new(
        code: impl Into<String>,
        severity: ValidationSeverity,
        fields: Vec<(String, Value)>,
    ) -> Result<Self, ServiceValidationError> {
        let mut seen = HashSet::new();
        for (key, _) in &fields {
            if matches!(key.as_str(), "code" | "severity" | "metadata") || !seen.insert(key) {
                return Err(ServiceValidationError(format!(
                    "Invalid validation payload key ({key})."
                )));
            }
        }
        Ok(Self {
            code: code.into(),
            severity,
            fields,
            legacy_message: None,
        })
    }
    pub fn with_legacy_message(mut self, message: impl Into<String>) -> Self {
        self.legacy_message = Some(message.into());
        self
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn severity(&self) -> ValidationSeverity {
        self.severity
    }
    fn json(&self) -> Wire {
        let mut fields = vec![
            ("code".into(), Wire::text(&self.code)),
            ("severity".into(), Wire::text(self.severity.wire())),
        ];
        fields.extend(
            self.fields
                .iter()
                .map(|(key, value)| (key.clone(), Wire::from(value.clone()))),
        );
        Wire::Object(fields)
    }
    fn legacy_message(&self) -> String {
        self.legacy_message
            .clone()
            .unwrap_or_else(|| format!("Validation error of code ({}) occurred.", self.code))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceValidationResult {
    pub status: ServiceValidationStatus,
    pub issues: Vec<ServiceValidationIssue>,
}
impl ServiceValidationResult {
    pub fn to_json(&self) -> Value {
        serde_json::from_str(&self.to_pretty_json()).expect("owned validation JSON is valid")
    }
    pub fn to_pretty_json(&self) -> String {
        Wire::Object(vec![
            ("status".into(), Wire::text(self.status.wire())),
            (
                "issues".into(),
                Wire::Array(
                    self.issues
                        .iter()
                        .map(ServiceValidationIssue::json)
                        .collect(),
                ),
            ),
        ])
        .render()
    }
    pub fn to_legacy_pretty_json(&self) -> String {
        Wire::Object(vec![
            ("summary".into(), Wire::text(self.status.wire())),
            (
                "errors".into(),
                Wire::Array(
                    self.issues
                        .iter()
                        .map(|issue| Wire::text(issue.legacy_message()))
                        .collect(),
                ),
            ),
        ])
        .render()
    }
}
#[derive(Default)]
pub struct ServiceValidationBuilder {
    issues: Vec<ServiceValidationIssue>,
}
impl ServiceValidationBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_issue(mut self, issue: ServiceValidationIssue) -> Self {
        self.issues.push(issue);
        self
    }
    pub fn build(self) -> ServiceValidationResult {
        let status = if self
            .issues
            .iter()
            .any(|i| i.severity == ValidationSeverity::Error)
        {
            ServiceValidationStatus::Errors
        } else if self.issues.is_empty() {
            ServiceValidationStatus::Ok
        } else {
            ServiceValidationStatus::Warnings
        };
        ServiceValidationResult {
            status,
            issues: self.issues,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceValidationSummary {
    pub status: ServiceValidationStatus,
    pub error_count: Option<usize>,
    pub warning_count: Option<usize>,
    pub error_counts_by_code: Option<BTreeMap<String, usize>>,
    pub warning_counts_by_code: Option<BTreeMap<String, usize>>,
}
impl ServiceValidationSummary {
    /// The declared status is retained. Unsupported validation carries no counts;
    /// otherwise duplicate issue occurrences count individually in each severity.
    pub fn of(result: &ServiceValidationResult) -> Self {
        if result.status == ServiceValidationStatus::NotSupported {
            return Self {
                status: result.status,
                error_count: None,
                warning_count: None,
                error_counts_by_code: None,
                warning_counts_by_code: None,
            };
        }
        let mut errors = BTreeMap::new();
        let mut warnings = BTreeMap::new();
        for issue in &result.issues {
            let counts = match issue.severity {
                ValidationSeverity::Error => &mut errors,
                ValidationSeverity::Warning => &mut warnings,
            };
            *counts.entry(issue.code.clone()).or_insert(0) += 1;
        }
        Self {
            status: result.status,
            error_count: Some(errors.values().sum()),
            warning_count: Some(warnings.values().sum()),
            error_counts_by_code: Some(errors),
            warning_counts_by_code: Some(warnings),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceValidationIssueType {
    pub code: String,
    pub severity: ValidationSeverity,
    pub message: String,
}
impl ServiceValidationIssueType {
    pub fn to_pretty_json(&self) -> String {
        Wire::Object(vec![
            ("code".into(), Wire::text(&self.code)),
            ("severity".into(), Wire::text(self.severity.wire())),
            (
                "metadata".into(),
                Wire::Array(vec![Wire::Object(vec![
                    ("message".into(), Wire::text(&self.message)),
                    ("type".into(), Wire::text("Message")),
                ])]),
            ),
        ])
        .render()
    }
}
/// Legacy `Example` labels normalize to `Template`. Other labels are retained
/// for native forward compatibility; this is not Java enum discovery/parity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceModelMaturity {
    Template,
    Other(String),
}
impl ServiceModelMaturity {
    pub fn from_json(json: &str) -> Result<Self, ServiceValidationError> {
        let value: Value =
            serde_json::from_str(json).map_err(|e| ServiceValidationError(e.to_string()))?;
        match value {
            Value::String(s) if s == "Example" || s == "Template" => Ok(Self::Template),
            Value::String(s) => Ok(Self::Other(s)),
            _ => Err(ServiceValidationError(
                "Model maturity must be a JSON string.".into(),
            )),
        }
    }
    pub fn to_json(&self) -> Value {
        Value::String(match self {
            Self::Template => "Template".into(),
            Self::Other(s) => s.clone(),
        })
    }
}

// Generic ordered JSON renderer: objects use two-space lines, arrays use inline
// separators, and every scalar/key is escaped by serde_json. No fixture output
// or issue-code-specific formatting rules enter this serializer.
enum Wire {
    Scalar(Value),
    Object(Vec<(String, Wire)>),
    Array(Vec<Wire>),
}
impl From<Value> for Wire {
    fn from(value: Value) -> Self {
        match value {
            Value::Object(map) => {
                Self::Object(map.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
            Value::Array(values) => Self::Array(values.into_iter().map(Self::from).collect()),
            scalar => Self::Scalar(scalar),
        }
    }
}
impl Wire {
    fn text(text: impl Into<String>) -> Self {
        Self::Scalar(Value::String(text.into()))
    }
    fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }
    fn write(&self, out: &mut String, indent: usize) {
        match self {
            Self::Scalar(value) => {
                out.push_str(&serde_json::to_string(value).expect("JSON scalar serialization"))
            }
            Self::Object(fields) => {
                if fields.is_empty() {
                    out.push_str("{ }");
                    return;
                }
                out.push_str("{\n");
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    out.push_str(&"  ".repeat(indent + 1));
                    out.push_str(&serde_json::to_string(key).expect("JSON key serialization"));
                    out.push_str(" : ");
                    value.write(out, indent + 1);
                }
                out.push('\n');
                out.push_str(&"  ".repeat(indent));
                out.push('}');
            }
            Self::Array(values) => {
                out.push_str("[ ");
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    value.write(out, indent);
                }
                if !values.is_empty() {
                    out.push(' ');
                }
                out.push(']');
            }
        }
    }
}

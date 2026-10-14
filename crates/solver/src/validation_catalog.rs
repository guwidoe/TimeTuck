//! Explicit typed validator registration and validation before native execution.
use crate::{
    ServiceValidationError, ServiceValidationResult, ServiceValidationStatus, ValidationSeverity,
};
use serde_json::{Value, json};
use std::{
    any::{TypeId, type_name},
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::Path,
};

/// Owned API metadata. Registration binds this description to an actual Rust type.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidationTypeDescription {
    pub code: String,
    pub severity: ValidationSeverity,
    pub metadata: Vec<Value>,
}
#[derive(Clone, Debug)]
struct RegisteredType {
    rust_type: TypeId,
    rust_name: &'static str,
    description: ValidationTypeDescription,
}
#[derive(Clone, Debug, Default)]
pub struct ValidationTypeCatalog {
    types: BTreeMap<String, RegisteredType>,
}
impl ValidationTypeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reject duplicate codes before replacing a previously registered type.
    /// Explicit registration replaces JVM scanning; no class loading is performed.
    pub fn register<I: 'static>(
        &mut self,
        description: ValidationTypeDescription,
    ) -> Result<(), ServiceValidationError> {
        if description.code.is_empty() {
            return Err(ServiceValidationError(
                "Validation issue code must not be empty.".into(),
            ));
        }
        if let Some(previous) = self.types.get(&description.code) {
            return Err(ServiceValidationError(format!(
                "IssueCode ({}) was found in 2 types: {} and {}.",
                description.code,
                previous.rust_name,
                type_name::<I>()
            )));
        }
        self.types.insert(
            description.code.clone(),
            RegisteredType {
                rust_type: TypeId::of::<I>(),
                rust_name: type_name::<I>(),
                description,
            },
        );
        Ok(())
    }
    pub fn descriptions(&self) -> impl ExactSizeIterator<Item = &ValidationTypeDescription> {
        self.types.values().map(|entry| &entry.description)
    }
    pub fn description_for<I: 'static>(&self) -> Option<&ValidationTypeDescription> {
        self.types
            .values()
            .find(|entry| entry.rust_type == TypeId::of::<I>())
            .map(|entry| &entry.description)
    }
    pub fn descriptor_json(&self) -> Value {
        Value::Array(self.descriptions().map(|entry| json!({
            "code": entry.code,
            "severity": match entry.severity { ValidationSeverity::Error => "ERROR", ValidationSeverity::Warning => "WARNING" },
            "metadata": entry.metadata,
        })).collect())
    }
    /// Write only to the explicitly supplied destination. Rust identities and callbacks
    /// remain runtime registrations and are not reconstructed from this descriptor.
    pub fn write_descriptor(&self, path: impl AsRef<Path>) -> Result<(), ServiceValidationError> {
        let bytes = serde_json::to_vec_pretty(&self.descriptor_json())
            .map_err(|e| ServiceValidationError(e.to_string()))?;
        fs::write(path, bytes).map_err(|e| ServiceValidationError(e.to_string()))
    }
    fn check_result(&self, result: &ServiceValidationResult) -> Result<(), ServiceValidationError> {
        for issue in &result.issues {
            let entry = self.types.get(issue.code()).ok_or_else(|| {
                ServiceValidationError(format!(
                    "Unregistered validation issue code ({}).",
                    issue.code()
                ))
            })?;
            if entry.description.severity != issue.severity() {
                return Err(ServiceValidationError(format!(
                    "Validation issue severity differs for ({}).",
                    issue.code()
                )));
            }
        }
        let expected = if result
            .issues
            .iter()
            .any(|i| i.severity() == ValidationSeverity::Error)
        {
            ServiceValidationStatus::Errors
        } else if result.issues.is_empty() {
            ServiceValidationStatus::Ok
        } else {
            ServiceValidationStatus::Warnings
        };
        if result.status != expected {
            return Err(ServiceValidationError(
                "Validator result status disagrees with its issues.".into(),
            ));
        }
        Ok(())
    }
}

/// Validator callbacks inspect the actual caller model and configuration.
/// A valid result (including warnings) is required before the execution callback.
pub struct ValidatedModelRunner<M, C, F> {
    catalog: ValidationTypeCatalog,
    validator: F,
    marker: std::marker::PhantomData<fn(&M, &C)>,
}
impl<M, C, F> ValidatedModelRunner<M, C, F>
where
    F: Fn(&M, &C) -> Result<ServiceValidationResult, ServiceValidationError>,
{
    pub fn new(catalog: ValidationTypeCatalog, validator: F) -> Self {
        Self {
            catalog,
            validator,
            marker: std::marker::PhantomData,
        }
    }
    pub fn catalog(&self) -> &ValidationTypeCatalog {
        &self.catalog
    }
    pub fn validate(
        &self,
        model: &M,
        config: &C,
    ) -> Result<ServiceValidationResult, ServiceValidationError> {
        let result = (self.validator)(model, config)?;
        self.catalog.check_result(&result)?;
        Ok(result)
    }
    pub fn run<O, E>(
        &self,
        model: &M,
        config: &C,
        execute: impl FnOnce(&M, &C) -> Result<O, E>,
    ) -> Result<ValidatedModelOutput<O>, ValidatedModelRunError<E>> {
        let validation = self
            .validate(model, config)
            .map_err(ValidatedModelRunError::Validation)?;
        if validation.status == ServiceValidationStatus::Errors {
            return Err(ValidatedModelRunError::Rejected(validation));
        }
        let output = execute(model, config).map_err(ValidatedModelRunError::Execution)?;
        Ok(ValidatedModelOutput { output, validation })
    }
}
#[derive(Debug)]
pub struct ValidatedModelOutput<O> {
    pub output: O,
    pub validation: ServiceValidationResult,
}
#[derive(Debug)]
pub enum ValidatedModelRunError<E> {
    Validation(ServiceValidationError),
    Rejected(ServiceValidationResult),
    Execution(E),
}
impl<E: fmt::Display> fmt::Display for ValidatedModelRunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => write!(f, "Validation failed: {e}"),
            Self::Rejected(result) => write!(
                f,
                "Model rejected with {} validation issues.",
                result.issues.len()
            ),
            Self::Execution(e) => write!(f, "Validated model execution failed: {e}"),
        }
    }
}
impl<E: Error + 'static> Error for ValidatedModelRunError<E> {}

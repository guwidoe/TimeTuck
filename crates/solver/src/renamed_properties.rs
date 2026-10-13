//! Owned property resolution; native callbacks replace framework logging.
use crate::{EmployeeRouteDispatcher, ServiceDescriptorError, ServiceModelDescriptor};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnedPropertySource {
    values: BTreeMap<String, String>,
}
impl OwnedPropertySource {
    pub fn new(properties: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            values: properties.into_iter().collect(),
        }
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyFallbackWarning {
    pub canonical: String,
    pub legacy: String,
    pub message: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyFallbackRules {
    exact: BTreeMap<String, Vec<String>>,
    prefixes: BTreeMap<String, Vec<String>>,
}
impl PropertyFallbackRules {
    pub fn new() -> Self {
        Self::default()
    }
    /// Alias order is caller supplied. Re-registering a key replaces its aliases.
    pub fn exact(mut self, canonical: impl Into<String>, aliases: Vec<String>) -> Self {
        self.exact.insert(canonical.into(), aliases);
        self
    }
    /// Prefixes match the exact key or a dot-delimited descendant. Longest wins.
    pub fn prefix(mut self, canonical: impl Into<String>, aliases: Vec<String>) -> Self {
        self.prefixes.insert(canonical.into(), aliases);
        self
    }
    /// Only mappings visible in pinned Community TEST fixtures are registered.
    pub fn community_test_profile() -> Self {
        let mut rules = Self::new();
        for (name, aliases) in [
            ("timefold.model.name", vec!["timefold.application.name"]),
            (
                "timefold.model.id",
                vec!["timefold.application.id", "ai.timefold.platform.model"],
            ),
            (
                "timefold.model.api-version",
                vec![
                    "timefold.application.version",
                    "ai.timefold.platform.model-version",
                ],
            ),
            (
                "timefold.model.rest-resource",
                vec!["ai.timefold.platform.model-resource"],
            ),
            (
                "timefold.model.schema.validation.enable",
                vec!["ai.timefold.platform.models.validation.enable"],
            ),
        ] {
            rules = rules.exact(name, aliases.into_iter().map(str::to_owned).collect());
        }
        for (name, alias) in [
            ("timefold.platform", "ai.timefold.platform"),
            (
                "timefold.model.default-config",
                "ai.timefold.model.default-config",
            ),
            (
                "timefold.model.termination",
                "ai.timefold.platform.termination",
            ),
            ("timefold.model.contact", "timefold.application.contact"),
            ("timefold.model.rest", "timefold.rest"),
            ("timefold.storage", "ai.timefold.storage"),
        ] {
            rules = rules.prefix(name, vec![alias.to_owned()]);
        }
        rules
    }
    pub fn resolve<'a>(
        &self,
        source: &'a OwnedPropertySource,
        name: &str,
        sink: &mut impl FnMut(PropertyFallbackWarning),
    ) -> Option<&'a str> {
        if let Some(value) = source.get(name) {
            return Some(value);
        }
        let (aliases, suffix) = if let Some(aliases) = self.exact.get(name) {
            (aliases, "")
        } else {
            let (prefix, aliases) = self
                .prefixes
                .iter()
                .filter(|(prefix, _)| {
                    name == prefix.as_str()
                        || name
                            .strip_prefix(prefix.as_str())
                            .is_some_and(|suffix| suffix.starts_with('.'))
                })
                .max_by_key(|(prefix, _)| prefix.len())?;
            (aliases, &name[prefix.len()..])
        };
        for alias in aliases {
            let legacy = format!("{alias}{suffix}");
            if let Some(value) = source.get(&legacy) {
                sink(PropertyFallbackWarning {
                    canonical: name.to_owned(),
                    message: format!("Deprecated configuration key '{legacy}'; use '{name}'."),
                    legacy,
                });
                return Some(value);
            }
        }
        None
    }
    /// Validation completes before a dispatcher exists or a command is enqueued.
    pub fn employee_dispatcher(
        &self,
        source: &OwnedPropertySource,
        capacity: usize,
        sink: &mut impl FnMut(PropertyFallbackWarning),
    ) -> Result<EmployeeRouteDispatcher, ServiceDescriptorError> {
        let id = self.resolve(source, "timefold.model.id", sink);
        let version = self.resolve(source, "timefold.model.api-version", sink);
        let path = self.resolve(source, "timefold.model.rest-resource", sink);
        ServiceModelDescriptor::new(id, version, path.unwrap_or(""))
            .and_then(|descriptor| descriptor.employee_dispatcher(capacity))
    }
}

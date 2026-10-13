//! Optional provenance display for caller-supplied named version modules.
//! Names describe supplied metadata; no editions/services/artifacts are detected.
use crate::{VersionMetadata, VersionReporter};
use std::{collections::BTreeSet, error::Error, fmt};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleBannerError {
    InvalidProductName,
    InvalidModuleName,
    DuplicateModule(String),
}
impl fmt::Display for ModuleBannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProductName => f.write_str("invalid product display name"),
            Self::InvalidModuleName => f.write_str("module name must be an ASCII identifier"),
            Self::DuplicateModule(name) => write!(f, "duplicate provenance module {name}"),
        }
    }
}
impl Error for ModuleBannerError {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionModuleProvenance {
    name: String,
    reporter: VersionReporter,
}
impl VersionModuleProvenance {
    /// Resolve the actual optional resource before using explicit fallback data.
    /// Type/ref validation occurs through VersionMetadata, never arbitrary strings.
    pub fn resolve(
        name: impl Into<String>,
        resource: &str,
        load: impl FnOnce(&str) -> Option<VersionMetadata>,
        fallback: Option<VersionMetadata>,
    ) -> Result<Self, ModuleBannerError> {
        let name = name.into();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(ModuleBannerError::InvalidModuleName);
        }
        Ok(Self {
            name,
            reporter: VersionReporter::resolve(resource, load, fallback),
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn reporter(&self) -> &VersionReporter {
        &self.reporter
    }
}
impl VersionReporter {
    /// Caller-order provenance suffix; modules lacking a known revision are
    /// omitted entirely. No empty parentheses or synthetic unknown refs appear.
    /// Product/version resolution uses the existing reporter unchanged.
    pub fn module_provenance_banner(
        &self,
        product: &str,
        modules: &[VersionModuleProvenance],
    ) -> Result<String, ModuleBannerError> {
        if product.is_empty()
            || product.trim() != product
            || product
                .chars()
                .any(|c| c.is_control() || matches!(c, '(' | ')' | ','))
        {
            return Err(ModuleBannerError::InvalidProductName);
        }
        let mut seen = BTreeSet::new();
        let mut refs = vec![];
        for module in modules {
            if !seen.insert(module.name()) {
                return Err(ModuleBannerError::DuplicateModule(module.name.clone()));
            }
            if let Some(revision) = module.reporter.revision() {
                refs.push(format!("{} {revision}", module.name()));
            }
        }
        let banner = self.banner(product);
        Ok(if refs.is_empty() {
            banner
        } else {
            format!("{banner} ({})", refs.join(", "))
        })
    }
}

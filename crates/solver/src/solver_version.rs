//! Native package/resource diagnostics, independent of JVM manifests and loaders.
use std::{error::Error, fmt};

pub const COMMUNITY_EDITION: &str = "TimeTuck Community Edition";
pub const DEVELOPMENT_SNAPSHOT: &str = "Development Snapshot";

/// Remove only a trailing, lowercase release-candidate number.
pub fn strip_release_candidate(version: Option<&str>) -> Option<&str> {
    version.map(|version| match version.rsplit_once("-rc-") {
        Some((base, number))
            if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            base
        }
        _ => version,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionMetadata {
    version: String,
    revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionMetadataError(&'static str);
impl fmt::Display for VersionMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for VersionMetadataError {}

impl VersionMetadata {
    pub fn new(
        version: impl Into<String>,
        revision: Option<String>,
    ) -> Result<Self, VersionMetadataError> {
        let version = version.into();
        if version.is_empty() || version.chars().any(char::is_whitespace) {
            return Err(VersionMetadataError("version must be a nonempty token"));
        }
        if revision.as_deref().is_some_and(|s| {
            !(7..=40).contains(&s.len())
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(VersionMetadataError(
                "revision must be 7..40 lowercase hexadecimal characters",
            ));
        }
        Ok(Self { version, revision })
    }
    /// Actual Cargo package metadata. A revision is absent unless independently supplied.
    pub fn package() -> Self {
        Self::new(env!("CARGO_PKG_VERSION"), None).expect("Cargo package version is a token")
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
}

/// Resolve an optional named resource before consulting explicit fallback metadata.
/// The loader executes once; callers control I/O and error handling before providing
/// metadata. Native registrations replace JVM package/classloader discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionReporter {
    metadata: Option<VersionMetadata>,
}
impl VersionReporter {
    pub fn resolve(
        resource: &str,
        load: impl FnOnce(&str) -> Option<VersionMetadata>,
        fallback: Option<VersionMetadata>,
    ) -> Self {
        Self {
            metadata: load(resource).or(fallback),
        }
    }
    pub fn package() -> Self {
        Self {
            metadata: Some(VersionMetadata::package()),
        }
    }
    pub fn bare_version(&self) -> &str {
        self.metadata.as_ref().map_or("SNAPSHOT", |m| {
            strip_release_candidate(Some(m.version())).unwrap()
        })
    }
    pub fn revision(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(VersionMetadata::revision)
    }
    pub fn banner(&self, edition: &str) -> String {
        match &self.metadata {
            Some(_) => format!("{edition} v{}", self.bare_version()),
            None => format!("{edition} {DEVELOPMENT_SNAPSHOT}"),
        }
    }
    pub fn community_banner(&self, revision: Option<&str>) -> String {
        let banner = self.banner(COMMUNITY_EDITION);
        revision.map_or_else(
            || banner.clone(),
            |revision| format!("{banner} ({revision})"),
        )
    }
    pub fn identify(&self) -> String {
        self.community_banner(self.revision())
    }
}

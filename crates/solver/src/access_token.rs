//! Native access-token resolution from explicit environment/settings snapshots.
use std::{error::Error, fmt};
pub const DEFAULT_ACCESS_TOKEN_SERVER_ID: &str = "timefold-platform";
pub const MAVEN_ENCRYPTION_GUIDE_URL: &str =
    "https://maven.apache.org/guides/mini/guide-encryption.html";
/// Caller-owned secret. Debug never emits its value; exposure is explicit.
#[derive(Clone, Eq, PartialEq)]
pub struct AccessTokenSecret(String);
impl AccessTokenSecret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for AccessTokenSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessTokenSecret([REDACTED])")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessTokenServer {
    pub id: String,
    pub password: Option<AccessTokenSecret>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccessTokenSettings {
    pub servers: Vec<AccessTokenServer>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessTokenSeverity {
    Warning,
    Error,
    Fatal,
}
/// Decrypter details must not contain unknown secrets. Resolver additionally
/// redacts the stored/decrypted password (including its trimmed representation).
#[derive(Clone, Debug)]
pub struct AccessTokenProblem {
    pub severity: AccessTokenSeverity,
    pub message: String,
}
#[derive(Clone, Debug, Default)]
pub struct AccessTokenDecryption {
    pub server: Option<AccessTokenServer>,
    pub problems: Vec<AccessTokenProblem>,
}
/// Real caller decryption boundary. Native resolver supplies a private cloned
/// server, never shared session settings. This trait does not implement Maven
/// ciphertext/master-password cryptography; a genuine backend is caller-owned.
pub trait AccessTokenDecrypter {
    fn decrypt(&self, server: &AccessTokenServer) -> AccessTokenDecryption;
}
impl<F: Fn(&AccessTokenServer) -> AccessTokenDecryption> AccessTokenDecrypter for F {
    fn decrypt(&self, server: &AccessTokenServer) -> AccessTokenDecryption {
        self(server)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessTokenWarning {
    pub message: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessTokenResolution {
    pub token: Option<AccessTokenSecret>,
    pub warnings: Vec<AccessTokenWarning>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessTokenError {
    message: String,
    warnings: Vec<AccessTokenWarning>,
}
impl fmt::Display for AccessTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for AccessTokenError {}
impl AccessTokenError {
    pub fn warnings(&self) -> &[AccessTokenWarning] {
        &self.warnings
    }
}
/// Decoration recognition is independent of base64 or decryption. At least one
/// character must occur between an opening brace and a closing brace whose
/// predecessor is not a backslash. Prefix/suffix comments are accepted. Opening
/// escapes alone do not hide the decoration (verified against public Plexus API).
/// All Java line terminators reject, deliberately stricter than Maven's one
/// leading-line-terminator exception. Work is linear and allocates nothing.
pub fn is_access_token_encrypted(value: &str) -> bool {
    if value
        .chars()
        .any(|c| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'))
    {
        return false;
    }
    let mut opening = None;
    let mut previous = None;
    for (index, c) in value.chars().enumerate() {
        if c == '{' && opening.is_none() {
            opening = Some(index);
        }
        if c == '}' && opening.is_some_and(|open| index > open + 1) && previous != Some('\\') {
            return true;
        }
        previous = Some(c);
    }
    false
}
fn redact(message: &str, secrets: &[&str]) -> String {
    let mut result = message.to_owned();
    for secret in secrets {
        if !secret.is_empty() {
            result = result.replace(secret, "[REDACTED]");
            let trimmed = secret.trim();
            if !trimmed.is_empty() {
                result = result.replace(trimmed, "[REDACTED]");
            }
        }
    }
    result
}
/// Reusable immutable provider configuration. Environment/settings are explicit
/// borrowed snapshots; no process environment reads, filesystem changes or logs
/// occur implicitly. Every resolution owns safe structured warnings. Missing or
/// blank server IDs normalize to the default before both lookup and diagnostics.
/// Secret trimming/ownership is native; no Maven session/Java plugin ABI implied.
pub struct AccessTokenProvider<'a> {
    settings: Option<&'a AccessTokenSettings>,
    decrypter: Option<&'a dyn AccessTokenDecrypter>,
    server_id: String,
}
impl<'a> AccessTokenProvider<'a> {
    pub fn new(
        settings: Option<&'a AccessTokenSettings>,
        decrypter: Option<&'a dyn AccessTokenDecrypter>,
        server_id: Option<&str>,
    ) -> Self {
        let server_id = server_id
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(DEFAULT_ACCESS_TOKEN_SERVER_ID)
            .to_owned();
        Self {
            settings,
            decrypter,
            server_id,
        }
    }
    pub fn server_id(&self) -> &str {
        &self.server_id
    }
    pub fn resolve(
        &self,
        environment: Option<&AccessTokenSecret>,
    ) -> Result<AccessTokenResolution, AccessTokenError> {
        let mut warnings = vec![];
        if let Some(token) = environment
            .map(AccessTokenSecret::expose)
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            return Ok(AccessTokenResolution {
                token: Some(AccessTokenSecret::new(token)),
                warnings,
            });
        }
        let Some(server) = self
            .settings
            .and_then(|s| s.servers.iter().find(|s| s.id == self.server_id))
        else {
            return Ok(AccessTokenResolution {
                token: None,
                warnings,
            });
        };
        let Some(stored) = server
            .password
            .as_ref()
            .map(AccessTokenSecret::expose)
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            warnings.push(AccessTokenWarning {
                message: format!("Server '{}' has no <password>.", self.server_id),
            });
            return Ok(AccessTokenResolution {
                token: None,
                warnings,
            });
        };
        let encrypted = is_access_token_encrypted(stored);
        let result = self.decrypter.map(|d| d.decrypt(&server.clone()));
        let decrypted = result
            .as_ref()
            .and_then(|r| r.server.as_ref())
            .and_then(|s| s.password.as_ref())
            .map(AccessTokenSecret::expose);
        let original = server
            .password
            .as_ref()
            .map(AccessTokenSecret::expose)
            .unwrap_or("");
        let secrets = [original, stored, decrypted.unwrap_or("")];
        if let Some(result) = &result {
            let mut errors = vec![];
            for problem in &result.problems {
                let message = redact(&problem.message, &secrets);
                if problem.severity == AccessTokenSeverity::Warning {
                    warnings.push(AccessTokenWarning { message });
                } else {
                    errors.push(message);
                }
            }
            if !errors.is_empty() {
                return Err(AccessTokenError {
                    message: format!(
                        "Unable to decrypt the personal access token in server '{}': {}. Check ~/.m2/settings-security.xml and mvn --encrypt-password; see {}",
                        self.server_id,
                        errors.join("; "),
                        MAVEN_ENCRYPTION_GUIDE_URL
                    ),
                    warnings,
                });
            }
        }
        let token = decrypted.map(str::trim).filter(|s| !s.is_empty());
        if encrypted && (token.is_none() || token == Some(stored)) {
            return Err(AccessTokenError {
                message: format!(
                    "The personal access token in server '{}' could not be decrypted and is still in its encrypted form. Check ~/.m2/settings-security.xml and mvn --encrypt-password; see {}",
                    self.server_id, MAVEN_ENCRYPTION_GUIDE_URL
                ),
                warnings,
            });
        }
        if !encrypted {
            warnings.push(AccessTokenWarning{message:format!("The personal access token in server '{}' is stored unencrypted; use mvn --encrypt-password. See {}",self.server_id,MAVEN_ENCRYPTION_GUIDE_URL)});
        }
        Ok(AccessTokenResolution {
            token: Some(AccessTokenSecret::new(token.unwrap_or(stored))),
            warnings,
        })
    }
}

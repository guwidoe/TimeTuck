// Adapted Community AccessTokenProviderTest TEST fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
pub const TOKEN: &str = "dummy-access-token";
pub fn settings(id: &str, password: Option<&str>) -> AccessTokenSettings {
    AccessTokenSettings {
        servers: vec![AccessTokenServer {
            id: id.into(),
            password: password.map(AccessTokenSecret::new),
        }],
    }
}
pub fn plain(server: &AccessTokenServer) -> AccessTokenDecryption {
    AccessTokenDecryption {
        server: Some(server.clone()),
        problems: vec![],
    }
}
pub fn provider<'a>(
    s: Option<&'a AccessTokenSettings>,
    d: Option<&'a dyn AccessTokenDecrypter>,
    id: Option<&str>,
) -> AccessTokenProvider<'a> {
    AccessTokenProvider::new(s, d, id)
}
pub fn token(result: &AccessTokenResolution) -> Option<&str> {
    result.token.as_ref().map(AccessTokenSecret::expose)
}
pub fn warning(result: &AccessTokenResolution, text: &str) -> bool {
    result.warnings.iter().any(|w| w.message.contains(text))
}
pub fn problem(severity: AccessTokenSeverity, message: &str) -> AccessTokenProblem {
    AccessTokenProblem {
        severity,
        message: message.into(),
    }
}

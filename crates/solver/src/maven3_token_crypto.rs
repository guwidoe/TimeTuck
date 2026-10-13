//! Opt-in legacy Maven3/Plexus2 decorated password interoperability.
//! AES-CBC with one SHA256 derivation has no authentication. This is a legacy
//! format reader/writer, not a new secrets storage design or Maven4 cipher.
use crate::{
    AccessTokenDecrypter, AccessTokenDecryption, AccessTokenProblem, AccessTokenSecret,
    AccessTokenServer, AccessTokenSeverity, is_access_token_encrypted,
};
use aes::Aes128;
use base64::{Engine, engine::general_purpose::STANDARD};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, path::Path};
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maven3CryptoError {
    InvalidDecoration,
    InvalidPayload,
    DecryptionFailed,
    InvalidUtf8,
    InputTooLarge,
    InvalidSecuritySettings,
    MissingMaster,
    EntropyUnavailable,
    SecuritySettingsIo,
}
impl fmt::Display for Maven3CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDecoration => "invalid Maven3 password decoration",
            Self::InvalidPayload => "invalid Maven3 encrypted payload",
            Self::DecryptionFailed => "Maven3 password decryption failed",
            Self::InvalidUtf8 => "Maven3 decrypted password is not UTF-8",
            Self::InputTooLarge => "Maven3 password input exceeds limit",
            Self::InvalidSecuritySettings => "invalid settings-security.xml",
            Self::MissingMaster => "master password is not set",
            Self::EntropyUnavailable => "operating system randomness unavailable",
            Self::SecuritySettingsIo => "unable to read explicitly selected settings-security.xml",
        })
    }
}
impl Error for Maven3CryptoError {}
const LIMIT: usize = 1024 * 1024;
/// Exact TEST helper constant; master is encrypted under this fixed legacy key.
pub const MAVEN3_MASTER_ENCRYPTION_KEY: &str = "settings.security";
fn derive(password: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut digest = Sha256::new();
    digest.update(password.as_bytes());
    digest.update(salt);
    Zeroizing::new(digest.finalize().into())
}
fn body(value: &str) -> Result<&str, Maven3CryptoError> {
    if value.len() > LIMIT {
        return Err(Maven3CryptoError::InputTooLarge);
    }
    if !is_access_token_encrypted(value) {
        return Err(Maven3CryptoError::InvalidDecoration);
    }
    let start = value
        .find('{')
        .ok_or(Maven3CryptoError::InvalidDecoration)?
        + 1;
    let end = value[start..]
        .char_indices()
        .find(|(i, c)| *c == '}' && value.as_bytes()[start + i - 1] != b'\\')
        .map(|(i, _)| start + i)
        .ok_or(Maven3CryptoError::InvalidDecoration)?;
    Ok(&value[start..end])
}
/// Decode bounded decorated data including outside comments. No ciphertext or
/// password appears in errors. Derived key and temporary plaintext are zeroized;
/// returned secret uses the existing explicit-exposure/redacted Debug contract.
pub fn decrypt_maven3_password(
    value: &str,
    password: &AccessTokenSecret,
) -> Result<AccessTokenSecret, Maven3CryptoError> {
    if password.expose().len() > LIMIT {
        return Err(Maven3CryptoError::InputTooLarge);
    }
    let bytes = STANDARD
        .decode(body(value)?)
        .map_err(|_| Maven3CryptoError::InvalidPayload)?;
    if bytes.len() < 25 {
        return Err(Maven3CryptoError::InvalidPayload);
    }
    let padding = usize::from(bytes[8]);
    if padding > 7 || bytes.len() % 8 != 0 {
        return Err(Maven3CryptoError::InvalidPayload);
    }
    let end = bytes
        .len()
        .checked_sub(padding)
        .ok_or(Maven3CryptoError::InvalidPayload)?;
    if end <= 9 || (end - 9) % 16 != 0 {
        return Err(Maven3CryptoError::InvalidPayload);
    }
    let derived = derive(password.expose(), &bytes[..8]);
    let mut buffer = Zeroizing::new(bytes[9..end].to_vec());
    let plaintext = cbc::Decryptor::<Aes128>::new_from_slices(&derived[..16], &derived[16..])
        .map_err(|_| Maven3CryptoError::DecryptionFailed)?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| Maven3CryptoError::DecryptionFailed)?;
    let text = std::str::from_utf8(plaintext).map_err(|_| Maven3CryptoError::InvalidUtf8)?;
    Ok(AccessTokenSecret::new(text))
}
/// Interoperable legacy writer using fresh OS randomness for salt and filler.
/// This preserves the legacy unauthenticated format; it does not strengthen it.
pub fn encrypt_maven3_password(
    value: &AccessTokenSecret,
    password: &AccessTokenSecret,
) -> Result<AccessTokenSecret, Maven3CryptoError> {
    if value.expose().len() > LIMIT || password.expose().len() > LIMIT {
        return Err(Maven3CryptoError::InputTooLarge);
    }
    if value.expose().is_empty() {
        return Ok(AccessTokenSecret::new("{}"));
    }
    let mut entropy = [0u8; 15];
    getrandom::getrandom(&mut entropy).map_err(|_| Maven3CryptoError::EntropyUnavailable)?;
    let derived = derive(password.expose(), &entropy[..8]);
    let ciphertext = cbc::Encryptor::<Aes128>::new_from_slices(&derived[..16], &derived[16..])
        .map_err(|_| Maven3CryptoError::DecryptionFailed)?
        .encrypt_padded_vec_mut::<Pkcs7>(value.expose().as_bytes());
    let mut data = Vec::with_capacity(ciphertext.len() + 16);
    data.extend_from_slice(&entropy[..8]);
    data.push(7);
    data.extend(ciphertext);
    data.extend_from_slice(&entropy[8..]);
    let decorated = format!("{{{}}}", STANDARD.encode(data));
    if decorated.len() > LIMIT {
        return Err(Maven3CryptoError::InputTooLarge);
    }
    Ok(AccessTokenSecret::new(decorated))
}
/// An immutable, caller-selected encrypted-master snapshot. No implicit home,
/// environment, relocation or credential-file reads occur. Plain server values
/// (including escaped braces) pass through untouched, as Maven TESTs require.
pub struct Maven3SettingsDecrypter {
    master: AccessTokenSecret,
}
impl fmt::Debug for Maven3SettingsDecrypter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Maven3SettingsDecrypter([REDACTED])")
    }
}
impl Maven3SettingsDecrypter {
    pub fn from_security_xml(xml: &str) -> Result<Self, Maven3CryptoError> {
        if xml.len() > LIMIT {
            return Err(Maven3CryptoError::InputTooLarge);
        }
        let doc = roxmltree::Document::parse(xml)
            .map_err(|_| Maven3CryptoError::InvalidSecuritySettings)?;
        let root = doc.root_element();
        if !root.has_tag_name("settingsSecurity") || root.attributes().len() != 0 {
            return Err(Maven3CryptoError::InvalidSecuritySettings);
        }
        let elements: Vec<_> = root.children().filter(|n| n.is_element()).collect();
        if elements.is_empty() {
            return Err(Maven3CryptoError::MissingMaster);
        }
        if elements.len() != 1
            || !elements[0].has_tag_name("master")
            || elements[0].attributes().len() != 0
            || elements[0].children().any(|n| n.is_element())
        {
            return Err(Maven3CryptoError::InvalidSecuritySettings);
        }
        let master = elements[0]
            .text()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or(Maven3CryptoError::MissingMaster)?;
        let master = decrypt_maven3_password(
            master,
            &AccessTokenSecret::new(MAVEN3_MASTER_ENCRYPTION_KEY),
        )?;
        Ok(Self { master })
    }
    /// Reads only the explicit caller path. The caller owns trust/permissions.
    pub fn from_security_file(path: impl AsRef<Path>) -> Result<Self, Maven3CryptoError> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(|_| Maven3CryptoError::SecuritySettingsIo)?;
        let mut xml = String::new();
        file.take((LIMIT + 1) as u64)
            .read_to_string(&mut xml)
            .map_err(|_| Maven3CryptoError::SecuritySettingsIo)?;
        Self::from_security_xml(&xml)
    }
}
impl AccessTokenDecrypter for Maven3SettingsDecrypter {
    fn decrypt(&self, server: &AccessTokenServer) -> AccessTokenDecryption {
        let mut result = AccessTokenDecryption {
            server: Some(server.clone()),
            problems: vec![],
        };
        if let Some(stored) = &server.password
            && is_access_token_encrypted(stored.expose().trim())
        {
            match decrypt_maven3_password(stored.expose().trim(), &self.master) {
                Ok(secret) => result.server.as_mut().unwrap().password = Some(secret),
                Err(error) => result.problems.push(AccessTokenProblem {
                    severity: AccessTokenSeverity::Error,
                    message: error.to_string(),
                }),
            }
        }
        result
    }
}

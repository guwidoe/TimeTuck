// Source fake token/master fixture and opaque public-API outputs, Apache-2.0.
use timetuck::*;
pub fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!("token_crypto/plexus2_vectors.json")).unwrap()
}
pub fn source_settings(
    index: usize,
) -> (
    std::path::PathBuf,
    AccessTokenSettings,
    Maven3SettingsDecrypter,
) {
    let rows = vectors();
    let row = &rows[index];
    assert_eq!(row["kind"], "source");
    let path = std::env::temp_dir().join(format!(
        "timetuck-fake-token-{}-{index}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    let file = path.join("settings-security.xml");
    std::fs::write(
        &file,
        format!(
            "<settingsSecurity><master>{}</master></settingsSecurity>",
            row["encrypted_master"].as_str().unwrap()
        ),
    )
    .unwrap();
    let decrypter = Maven3SettingsDecrypter::from_security_file(&file).unwrap();
    let settings = AccessTokenSettings {
        servers: vec![AccessTokenServer {
            id: DEFAULT_ACCESS_TOKEN_SERVER_ID.into(),
            password: Some(AccessTokenSecret::new(row["ciphertext"].as_str().unwrap())),
        }],
    };
    (path, settings, decrypter)
}

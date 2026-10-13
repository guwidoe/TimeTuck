// Original independent legacy-protocol controls; no source method credit.
use base64::{Engine, engine::general_purpose::STANDARD};
use timetuck::*;
fn vectors() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/token_crypto/plexus2_vectors.json")).unwrap()
}
#[test]
fn native_decrypts_public_cipher_vectors_and_exports_fresh_native_writer_outputs() {
    let rows = vectors();
    let mut proof = String::new();
    for row in rows.as_array().unwrap() {
        let value = AccessTokenSecret::new(row["plaintext"].as_str().unwrap());
        let password = AccessTokenSecret::new(row["password"].as_str().unwrap());
        let observed =
            decrypt_maven3_password(row["ciphertext"].as_str().unwrap(), &password).unwrap();
        assert_eq!(observed, value);
        let encrypted = encrypt_maven3_password(&value, &password).unwrap();
        assert_eq!(
            decrypt_maven3_password(encrypted.expose(), &password).unwrap(),
            value
        );
        proof.push_str(&format!(
            "{}\t{}\t{}\n",
            STANDARD.encode(value.expose()),
            STANDARD.encode(password.expose()),
            STANDARD.encode(encrypted.expose())
        ));
    }
    if let Some(path) = std::env::var_os("TIMETUCK_CRYPTO_PROOF_PATH") {
        std::fs::write(path, proof).unwrap();
    }
}
#[test]
fn corruption_wrong_keys_and_invalid_master_are_redacted_and_fail_closed() {
    let rows = vectors();
    let row = &rows[0];
    let password = AccessTokenSecret::new("wrong-password");
    assert!(decrypt_maven3_password(row["ciphertext"].as_str().unwrap(), &password).is_err());
    let mut bytes = STANDARD
        .decode(
            &row["ciphertext"].as_str().unwrap()[1..row["ciphertext"].as_str().unwrap().len() - 1],
        )
        .unwrap();
    bytes[8] = 255;
    let bad = format!("{{{}}}", STANDARD.encode(bytes));
    assert!(decrypt_maven3_password(&bad, &AccessTokenSecret::new("the-master-password")).is_err());
    for xml in [
        "<settingsSecurity/>",
        "<settingsSecurity><master>{invalid}</master></settingsSecurity>",
        "<!DOCTYPE x [<!ENTITY z SYSTEM 'file:///fake'>]><settingsSecurity><master>&z;</master></settingsSecurity>",
        "<settingsSecurity><master>x</master><master>y</master></settingsSecurity>",
    ] {
        assert!(Maven3SettingsDecrypter::from_security_xml(xml).is_err());
    }
    let master = encrypt_maven3_password(
        &AccessTokenSecret::new("the-master-password"),
        &AccessTokenSecret::new(MAVEN3_MASTER_ENCRYPTION_KEY),
    )
    .unwrap();
    let decrypter = Maven3SettingsDecrypter::from_security_xml(&format!(
        "<settingsSecurity><master>{}</master></settingsSecurity>",
        master.expose()
    ))
    .unwrap();
    let settings = AccessTokenSettings {
        servers: vec![AccessTokenServer {
            id: DEFAULT_ACCESS_TOKEN_SERVER_ID.into(),
            password: Some(AccessTokenSecret::new(&bad)),
        }],
    };
    let error = AccessTokenProvider::new(Some(&settings), Some(&decrypter), None)
        .resolve(None)
        .unwrap_err()
        .to_string();
    assert!(!error.contains(&bad));
    assert!(!error.contains("the-master-password"));
    assert!(error.contains("Unable to decrypt"));
    assert!(!format!("{decrypter:?}").contains("the-master-password"));
}
#[test]
fn fresh_master_and_server_encryptions_use_entropy_and_real_settings_dispatch() {
    let master = AccessTokenSecret::new("synthetic-master-漢字");
    let key = AccessTokenSecret::new(MAVEN3_MASTER_ENCRYPTION_KEY);
    let a = encrypt_maven3_password(&master, &key).unwrap();
    let b = encrypt_maven3_password(&master, &key).unwrap();
    assert_ne!(a, b);
    let decrypter = Maven3SettingsDecrypter::from_security_xml(&format!(
        "<settingsSecurity><master>{}</master></settingsSecurity>",
        a.expose()
    ))
    .unwrap();
    for plain in [
        "dummy-access-token",
        "dummy-{not}-cipher-text",
        "dummy\\{abc\\}value",
    ] {
        let value = AccessTokenSecret::new(plain);
        let encrypted = if plain.contains('\\') {
            value.clone()
        } else {
            encrypt_maven3_password(&value, &master).unwrap()
        };
        let settings = AccessTokenSettings {
            servers: vec![AccessTokenServer {
                id: "my-platform".into(),
                password: Some(encrypted),
            }],
        };
        let before = settings.clone();
        let result =
            AccessTokenProvider::new(Some(&settings), Some(&decrypter), Some("my-platform"))
                .resolve(None)
                .unwrap();
        assert_eq!(result.token, Some(value));
        assert_eq!(settings, before);
        assert_eq!(!result.warnings.is_empty(), plain.contains('\\'));
    }
}

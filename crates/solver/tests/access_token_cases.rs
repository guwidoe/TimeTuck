// Full adapted pinned Community provider contracts (cryptography cases pending).
// Apache-2.0; compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/access_token_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn environment_token_takes_precedence_over_the_settings() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some("from-settings"));
    let env = AccessTokenSecret::new("  from-environment  ");
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), None)
                .resolve(Some(&env))
                .unwrap()
        ),
        Some("from-environment")
    );
}
#[test]
fn blank_environment_token_falls_back_to_the_settings() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some("from-settings"));
    let env = AccessTokenSecret::new("   ");
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), None)
                .resolve(Some(&env))
                .unwrap()
        ),
        Some("from-settings")
    );
}
#[test]
fn token_is_read_from_the_configured_server_id() {
    let s = settings("my-platform", Some("from-settings"));
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), Some("my-platform"))
                .resolve(None)
                .unwrap()
        ),
        Some("from-settings")
    );
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), None)
                .resolve(None)
                .unwrap()
        ),
        None
    );
}
#[test]
fn unencrypted_token_in_the_settings_is_used_but_reported() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(TOKEN));
    let r = provider(Some(&s), Some(&plain), None)
        .resolve(None)
        .unwrap();
    assert_eq!(token(&r), Some(TOKEN));
    assert!(warning(&r, "stored unencrypted"));
    assert!(warning(&r, "mvn --encrypt-password"));
}
#[test]
fn server_without_password_provides_no_token() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, None);
    let r = provider(Some(&s), Some(&plain), None)
        .resolve(None)
        .unwrap();
    assert_eq!(token(&r), None);
    assert!(warning(&r, "has no <password>"));
}
#[test]
fn no_token_at_all_resolves_to_null() {
    assert_eq!(
        token(&provider(None, Some(&plain), None).resolve(None).unwrap()),
        None
    );
    let s = AccessTokenSettings::default();
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), None)
                .resolve(None)
                .unwrap()
        ),
        None
    );
}
#[test]
fn failed_decryption_fails_with_the_encrypted_token_kept_out() {
    let encrypted = "{dummy-cipher-text}";
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(encrypted));
    let failing = |server: &AccessTokenServer| AccessTokenDecryption {
        server: Some(server.clone()),
        problems: vec![problem(
            AccessTokenSeverity::Error,
            "master password is not set",
        )],
    };
    let e = provider(Some(&s), Some(&failing), None)
        .resolve(None)
        .unwrap_err();
    let message = e.to_string();
    for part in [
        "Unable to decrypt the personal access token in server 'timefold-platform'",
        "master password is not set",
        "~/.m2/settings-security.xml",
        "mvn --encrypt-password",
        MAVEN_ENCRYPTION_GUIDE_URL,
    ] {
        assert!(message.contains(part), "missing {part}");
    }
    assert!(!message.contains(encrypted));
}
#[test]
fn decryption_warnings_are_reported_without_failing() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some("{encrypted}"));
    let warning_decrypter = |server: &AccessTokenServer| {
        let mut server = server.clone();
        server.password = Some(AccessTokenSecret::new(TOKEN));
        AccessTokenDecryption {
            server: Some(server),
            problems: vec![problem(
                AccessTokenSeverity::Warning,
                "deprecated encryption",
            )],
        }
    };
    let r = provider(Some(&s), Some(&warning_decrypter), None)
        .resolve(None)
        .unwrap();
    assert_eq!(token(&r), Some(TOKEN));
    assert!(warning(&r, "deprecated encryption"));
}
#[test]
fn escaped_braces_are_not_mistaken_for_cipher_text() {
    let escaped = r"dummy\{abc\}value";
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(escaped));
    let r = provider(Some(&s), Some(&plain), None)
        .resolve(None)
        .unwrap();
    assert_eq!(token(&r), Some(escaped));
    assert!(warning(&r, "stored unencrypted"));
}
#[test]
fn encrypted_token_without_a_decrypter_fails() {
    let encrypted = "{dummy-cipher-text}";
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(encrypted));
    let message = provider(Some(&s), None, None)
        .resolve(None)
        .unwrap_err()
        .to_string();
    for part in [
        "could not be decrypted",
        "still in its encrypted form",
        "mvn --encrypt-password",
    ] {
        assert!(message.contains(part));
    }
    assert!(!message.contains(encrypted));
}
#[test]
fn a_blank_server_id_reads_the_default_entry() {
    for id in [None, Some(""), Some("   ")] {
        let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(TOKEN));
        let p = provider(Some(&s), Some(&plain), id);
        assert_eq!(p.server_id(), DEFAULT_ACCESS_TOKEN_SERVER_ID);
        assert_eq!(token(&p.resolve(None).unwrap()), Some(TOKEN));
    }
}
#[test]
fn a_padded_server_id_reads_the_entry_it_names() {
    let s = settings("my-platform", Some(TOKEN));
    let p = provider(Some(&s), Some(&plain), Some("  my-platform  "));
    assert_eq!(p.server_id(), "my-platform");
    assert_eq!(token(&p.resolve(None).unwrap()), Some(TOKEN));
}
#[test]
fn whitespace_around_an_encrypted_token_does_not_hide_it() {
    let encrypted = "{dummy-cipher-text}";
    let s = settings(
        DEFAULT_ACCESS_TOKEN_SERVER_ID,
        Some(&format!("\n  {encrypted}\n  ")),
    );
    let message = provider(Some(&s), None, None)
        .resolve(None)
        .unwrap_err()
        .to_string();
    assert!(message.contains("could not be decrypted"));
    assert!(!message.contains(encrypted));
}
#[test]
fn reading_the_token_leaves_the_settings_alone() {
    let stored = format!("  {TOKEN}  ");
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(&stored));
    assert_eq!(
        token(
            &provider(Some(&s), Some(&plain), None)
                .resolve(None)
                .unwrap()
        ),
        Some(TOKEN)
    );
    assert_eq!(s.servers[0].password.as_ref().unwrap().expose(), stored);
}
#[test]
fn unencrypted_token_without_a_decrypter_is_still_usable() {
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(TOKEN));
    let r = provider(Some(&s), None, None).resolve(None).unwrap();
    assert_eq!(token(&r), Some(TOKEN));
    assert!(warning(&r, "stored unencrypted"));
}
#[test]
fn decrypter_that_leaves_the_token_encrypted_fails() {
    let encrypted = "{dummy-cipher-text}";
    let s = settings(DEFAULT_ACCESS_TOKEN_SERVER_ID, Some(encrypted));
    let nothing = |_: &AccessTokenServer| AccessTokenDecryption::default();
    let message = provider(Some(&s), Some(&nothing), None)
        .resolve(None)
        .unwrap_err()
        .to_string();
    assert!(message.contains("could not be decrypted"));
    assert!(!message.contains(encrypted));
}
#[test]
fn cipher_text_is_recognized_the_same_way_maven_does() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/access_token_cipher_recognition.json"
    ))
    .unwrap();
    let mut values = vec![String::new()];
    let mut previous = vec![String::new()];
    for _ in 1..=6 {
        previous = previous
            .iter()
            .flat_map(|s| ['{', '}', '\\', 'a'].map(|c| format!("{s}{c}")))
            .collect();
        values.extend(previous.iter().cloned());
    }
    let expected = reference["expected"].as_array().unwrap();
    assert_eq!(values.len(), 5461);
    assert_eq!(expected.len(), values.len());
    for (value, expected) in values.iter().zip(expected) {
        assert_eq!(
            is_access_token_encrypted(value),
            expected.as_bool().unwrap(),
            "{value:?}"
        );
    }
}
#[test]
fn a_value_holding_a_line_terminator_is_never_read_as_cipher_text() {
    for value in ["{a}\n", "{a\nb}", "\n{a}"] {
        assert!(!is_access_token_encrypted(value));
    } // Maven expectations false,false,true are retained from the TEST specification.
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/access_token_cipher_recognition.json"
    ))
    .unwrap();
    assert_eq!(
        reference["lineTerminatorExpected"],
        serde_json::json!([false, false, true])
    );
}

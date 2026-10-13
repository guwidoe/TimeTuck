//! Original multi-server/live callback, redaction, current settings and grammar controls.
use std::cell::Cell;
use timetuck::*;
fn server(id: &str, secret: &str) -> AccessTokenServer {
    AccessTokenServer {
        id: id.into(),
        password: Some(AccessTokenSecret::new(secret)),
    }
}
#[test]
fn live_decrypter_uses_only_selected_private_server_and_leaves_settings_unchanged() {
    let settings = AccessTokenSettings {
        servers: vec![
            server("first", "{first-secret}"),
            server("second", " {second-secret} "),
        ],
    };
    let snapshot = settings.clone();
    let calls = Cell::new(0);
    let decrypt = |source: &AccessTokenServer| {
        calls.set(calls.get() + 1);
        assert_eq!(source.id, "second");
        let mut result = source.clone();
        let value = source
            .password
            .as_ref()
            .unwrap()
            .expose()
            .trim()
            .replace("secret", "usable");
        result.password = Some(AccessTokenSecret::new(format!("token-{value}")));
        AccessTokenDecryption {
            server: Some(result),
            problems: vec![],
        }
    };
    let p = AccessTokenProvider::new(Some(&settings), Some(&decrypt), Some(" second "));
    let env = AccessTokenSecret::new(" env-token ");
    assert_eq!(
        p.resolve(Some(&env)).unwrap().token.unwrap().expose(),
        "env-token"
    );
    assert_eq!(calls.get(), 0);
    let r = p.resolve(None).unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(r.token.unwrap().expose(), "token-{second-usable}");
    assert!(r.warnings.is_empty());
    assert_eq!(settings, snapshot);
    assert!(!format!("{settings:?}").contains("second-secret"));
}
#[test]
fn errors_warnings_and_debug_keep_known_ciphertext_and_plain_tokens_out() {
    let stored = "{sensitive-stored-password}";
    let plain = "sensitive-plaintext-token";
    let settings = AccessTokenSettings {
        servers: vec![server(DEFAULT_ACCESS_TOKEN_SERVER_ID, stored)],
    };
    for severity in [AccessTokenSeverity::Error, AccessTokenSeverity::Fatal] {
        let decrypt = |s: &AccessTokenServer| {
            let mut server = s.clone();
            server.password = Some(AccessTokenSecret::new(plain));
            AccessTokenDecryption {
                server: Some(server),
                problems: vec![AccessTokenProblem {
                    severity,
                    message: format!("failed {stored} and {plain}"),
                }],
            }
        };
        let e = AccessTokenProvider::new(Some(&settings), Some(&decrypt), None)
            .resolve(None)
            .unwrap_err();
        for text in [e.to_string(), format!("{e:?}")] {
            assert!(!text.contains(stored));
            assert!(!text.contains(plain));
            assert!(text.contains("[REDACTED]"));
        }
    }
    let decrypt = |s: &AccessTokenServer| {
        let mut server = s.clone();
        server.password = Some(AccessTokenSecret::new(plain));
        AccessTokenDecryption {
            server: Some(server),
            problems: vec![AccessTokenProblem {
                severity: AccessTokenSeverity::Warning,
                message: format!("deprecated {stored} => {plain}"),
            }],
        }
    };
    let r = AccessTokenProvider::new(Some(&settings), Some(&decrypt), None)
        .resolve(None)
        .unwrap();
    assert_eq!(r.token.as_ref().unwrap().expose(), plain);
    let debug = format!("{r:?}");
    assert!(!debug.contains(stored));
    assert!(!debug.contains(plain));
    assert_eq!(r.warnings.len(), 1);
}
#[test]
fn unchanged_trimmed_cipher_and_absent_decryption_are_failures_but_plaintext_is_available() {
    for candidate in [None, Some(""), Some("  {cipher-data}  ")] {
        let settings = AccessTokenSettings {
            servers: vec![server(
                DEFAULT_ACCESS_TOKEN_SERVER_ID,
                "\n {cipher-data} \n",
            )],
        };
        let decrypt = |s: &AccessTokenServer| {
            let mut server = s.clone();
            server.password = candidate.map(AccessTokenSecret::new);
            AccessTokenDecryption {
                server: Some(server),
                problems: vec![],
            }
        };
        assert!(
            AccessTokenProvider::new(Some(&settings), Some(&decrypt), None)
                .resolve(None)
                .is_err()
        );
    }
    for value in ["plain", r"literal\{brace\}token", "line\ninside"] {
        let s = AccessTokenSettings {
            servers: vec![server(DEFAULT_ACCESS_TOKEN_SERVER_ID, value)],
        };
        assert_eq!(
            AccessTokenProvider::new(Some(&s), None, None)
                .resolve(None)
                .unwrap()
                .token
                .unwrap()
                .expose(),
            value
        );
    }
    let settings = AccessTokenSettings {
        servers: vec![server(DEFAULT_ACCESS_TOKEN_SERVER_ID, "  ")],
    };
    assert!(
        AccessTokenProvider::new(Some(&settings), None, None)
            .resolve(None)
            .unwrap()
            .token
            .is_none()
    );
}
#[test]
fn decoration_grammar_is_linear_general_and_conservative_for_line_terminators() {
    for value in [
        "{x}",
        "comment {x} tail",
        r"\{a}",
        "{{}",
        "{é}",
        "α{token}β",
    ] {
        assert!(is_access_token_encrypted(value), "{value:?}");
    }
    for value in [
        "{}",
        r"{a\}",
        r"dummy\{abc\}value",
        "unbraced",
        "{unfinished",
    ] {
        assert!(!is_access_token_encrypted(value), "{value:?}");
    }
    for line in ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'] {
        for value in [
            format!("{line}{{token}}"),
            format!("{{token}}{line}"),
            format!("{{a{line}b}}"),
        ] {
            assert!(!is_access_token_encrypted(&value));
        }
    }
    let value = format!(
        "{}{{cipher-data}}{}",
        "\\{".repeat(10000),
        "suffix".repeat(10000)
    );
    assert!(is_access_token_encrypted(&value));
}

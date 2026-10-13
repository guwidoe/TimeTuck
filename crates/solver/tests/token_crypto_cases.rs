// Complete pinned AccessTokenProvider TEST crypto assertions; Apache-2.0.
#[path = "fixtures/token_crypto_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn encrypted_token_in_the_settings_is_decrypted() {
    let (path, settings, decrypter) = source_settings(0);
    let before = settings.clone();
    let resolution = AccessTokenProvider::new(Some(&settings), Some(&decrypter), None)
        .resolve(None)
        .unwrap();
    assert_eq!(resolution.token.unwrap().expose(), "dummy-access-token");
    assert!(
        resolution
            .warnings
            .iter()
            .all(|w| !w.message.contains("unencrypted"))
    );
    assert_eq!(settings, before);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn encrypted_token_with_a_comment_is_not_reported_as_unencrypted() {
    let (path, settings, decrypter) = source_settings(1);
    assert!(
        settings.servers[0]
            .password
            .as_ref()
            .unwrap()
            .expose()
            .starts_with("rotate before 2026-12-01 ")
    );
    let resolution = AccessTokenProvider::new(Some(&settings), Some(&decrypter), None)
        .resolve(None)
        .unwrap();
    assert_eq!(resolution.token.unwrap().expose(), "dummy-access-token");
    assert!(
        resolution
            .warnings
            .iter()
            .all(|w| !w.message.contains("unencrypted"))
    );
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn decrypted_token_may_contain_curly_braces() {
    let (path, settings, decrypter) = source_settings(2);
    let resolution = AccessTokenProvider::new(Some(&settings), Some(&decrypter), None)
        .resolve(None)
        .unwrap();
    assert_eq!(
        resolution.token.unwrap().expose(),
        "dummy-{not}-cipher-text"
    );
    std::fs::remove_dir_all(path).unwrap();
}

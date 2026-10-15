// SPDX-License-Identifier: MPL-2.0
use super::{
    GoogleOAuth, GoogleProfile, InstalledCredentials, canonical_scopes, ensure_expected_subject,
    granted_scopes, parse_callback, validate_revocation_response,
};
use reqwest::StatusCode;
use reqwest::blocking::Client;

#[test]
fn parses_google_redirect_url() {
    let callback =
        parse_callback("http://localhost/?code=abc123&scope=email&state=state-value").unwrap();
    assert_eq!(callback.code, "abc123");
    assert_eq!(callback.state, "state-value");
}

#[test]
fn rejects_google_error_redirect() {
    let error = parse_callback("http://localhost/?error=access_denied&state=x").unwrap_err();
    assert!(error.to_string().contains("access_denied"));
}

#[test]
fn authorization_url_uses_dynamic_redirect_uri() {
    let mut oauth = GoogleOAuth {
        credentials: InstalledCredentials {
            client_id: "client".into(),
            client_secret: "secret".into(),
            auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
        },
        client: Client::new(),
        verifier: None,
        state: None,
        redirect_uri: "http://localhost".into(),
    };
    oauth.set_redirect_uri("http://127.0.0.1:43123/oauth2/callback");
    let url = oauth.authorization_url().unwrap();
    assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43123%2Foauth2%2Fcallback"));
    assert!(url.contains(
            "scope=openid+email+profile+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.readonly+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.modify"
        ));
}

#[test]
fn canonicalizes_and_deduplicates_granted_scopes() {
    assert_eq!(
        canonical_scopes("profile openid profile https://example.test/scope").unwrap(),
        vec!["https://example.test/scope", "openid", "profile"]
    );
}

#[test]
fn rejects_empty_granted_scope_sets() {
    assert!(canonical_scopes(" \t\n").is_err());
}

#[test]
fn rejects_missing_granted_scopes() {
    assert!(granted_scopes(None).is_err());
}

#[test]
fn reads_granted_scopes_from_the_token_response() {
    let token: super::TokenResponse = serde_json::from_str(
            r#"{
                "access_token": "access-token",
                "refresh_token": "refresh-token",
                "scope": "profile openid https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.modify"
            }"#,
        )
        .unwrap();
    assert_eq!(
        granted_scopes(token.scope.as_deref()).unwrap(),
        vec![
            "https://www.googleapis.com/auth/gmail.modify",
            "https://www.googleapis.com/auth/gmail.readonly",
            "openid",
            "profile"
        ]
    );
}

#[test]
fn rejects_reauthentication_subject_mismatch() {
    let profile = GoogleProfile {
        sub: "returned-subject".into(),
        email: "returned@example.com".into(),
        name: None,
    };
    let error = ensure_expected_subject(&profile, Some("selected-subject")).unwrap_err();
    assert!(error.to_string().contains("does not match"));
}

#[test]
fn treats_success_and_already_revoked_tokens_as_terminal() {
    assert!(validate_revocation_response(StatusCode::OK, None).is_ok());
    assert!(validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_token")).is_ok());
}

#[test]
fn rejects_other_revocation_failures_without_exposing_tokens() {
    let error =
        validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_request")).unwrap_err();
    assert!(error.to_string().contains("invalid_request"));
    assert!(!error.to_string().contains("refresh-token"));
}

#[test]
fn uses_current_keyring_coordinates_for_missing_or_malformed_references() {
    assert_eq!(
        super::keyring_coordinates(None, "subject"),
        ("arqen".into(), "subject".into())
    );
    assert_eq!(
        super::keyring_coordinates(Some("keyring:legacy:subject"), "fallback"),
        ("legacy".into(), "subject".into())
    );
    assert_eq!(
        super::keyring_coordinates(Some("not-a-reference"), "fallback"),
        ("arqen".into(), "fallback".into())
    );
}

fn probe_directory() -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("arqen-probe-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn write_client_credentials(directory: &std::path::Path, token_uri: &str) -> std::path::PathBuf {
    let path = directory.join("google-client-secret.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"installed":{{"client_id":"client","client_secret":"secret","auth_uri":"https://accounts.google.com/o/oauth2/v2/auth","token_uri":"{token_uri}"}}}}"#
        ),
    )
    .unwrap();
    path
}

#[test]
fn token_endpoint_probe_accepts_a_reachable_endpoint() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let directory = probe_directory();
    let credentials =
        write_client_credentials(&directory, &format!("http://127.0.0.1:{port}/token"));

    assert!(super::probe_token_endpoint(&credentials).is_ok());

    // Hold the listener until after the probe so the port cannot be reused.
    drop(listener);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn token_endpoint_probe_reports_a_refused_connection_as_unreachable() {
    let reserved = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let directory = probe_directory();
    let credentials =
        write_client_credentials(&directory, &format!("http://127.0.0.1:{port}/token"));

    let error = super::probe_token_endpoint(&credentials).unwrap_err();

    assert!(super::is_token_endpoint_unreachable(&error));
    assert!(error.to_string().contains("could not be reached"));
    assert!(!error.to_string().contains("secret"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn token_endpoint_probe_reports_a_failed_lookup_as_unreachable() {
    // This host's resolver wildcards and search-suffixes unknown names, so a
    // short bogus host still resolves. An over-long label is rejected by
    // resolution itself, which makes the lookup-failure branch deterministic
    // whatever the network's resolver does.
    let unresolvable = format!("https://{}.invalid/token", "a".repeat(300));
    let directory = probe_directory();
    let credentials = write_client_credentials(&directory, &unresolvable);

    let error = super::probe_token_endpoint(&credentials).unwrap_err();

    assert!(super::is_token_endpoint_unreachable(&error));
    assert!(error.to_string().contains("name resolution failed"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn token_endpoint_probe_rejects_a_malformed_token_endpoint() {
    let directory = probe_directory();
    let credentials = write_client_credentials(&directory, "not-a-url");

    let error = super::probe_token_endpoint(&credentials).unwrap_err();

    // A misconfigured client JSON is a configuration error, not a reachability
    // failure, so readiness keeps reporting it as an unavailable credential.
    assert!(!super::is_token_endpoint_unreachable(&error));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn token_endpoint_probe_fails_closed_when_client_credentials_are_missing() {
    let directory = probe_directory();

    let error = super::probe_token_endpoint(directory.join("absent.json")).unwrap_err();

    // A missing client secret is not a reachability failure: readiness must keep
    // reporting it as an unavailable credential instead.
    assert!(!super::is_token_endpoint_unreachable(&error));
    assert!(error.to_string().contains("absent.json"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(test)]
mod tests {
    use super::{
        GoogleOAuth, GoogleProfile, InstalledCredentials, canonical_scopes,
        ensure_expected_subject, granted_scopes, parse_callback, validate_revocation_response,
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
        assert!(
            validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_token")).is_ok()
        );
    }

    #[test]
    fn rejects_other_revocation_failures_without_exposing_tokens() {
        let error = validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_request"))
            .unwrap_err();
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
}

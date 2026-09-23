#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::prepare_socket_path;
    use super::{read_bounded_line, target_ineligibility};
    use crate::{auth::GoogleTokenError, gmail::GmailApiError};
    use crate::{Account, ConnectionState, GMAIL_READONLY_SCOPE};
    use reqwest::StatusCode;
    use std::io::BufReader;

    fn account() -> Account {
        Account {
            id: "account".into(),
            subject: "subject".into(),
            email: "user@example.com".into(),
            display_name: None,
            token_key: Some("keyring:arqen:subject".into()),
            granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into()]),
            connection_state: ConnectionState::Connected,
        }
    }

    #[test]
    fn target_eligibility_requires_connected_verified_gmail_access() {
        let eligible = account();
        assert_eq!(target_ineligibility(&eligible), None);
        let mut disconnected = eligible.clone();
        disconnected.connection_state = ConnectionState::Disconnected;
        assert!(target_ineligibility(&disconnected).is_some());
        let mut unverified = eligible.clone();
        unverified.granted_scopes = None;
        assert!(target_ineligibility(&unverified).is_some());
    }

    #[test]
    fn bounded_request_reader_rejects_unterminated_oversized_input() {
        let oversized = vec![b'x'; super::MAX_REQUEST_BYTES + 1];
        let error = read_bounded_line(
            &mut BufReader::new(oversized.as_slice()),
            super::MAX_REQUEST_BYTES,
        )
        .unwrap_err();
        assert!(error.to_string().contains("too large"));
    }

    #[test]
    fn bounded_request_reader_accepts_a_single_json_line() {
        let mut reader = BufReader::new(
            br#"{"operation":"list_emails"}
"#
            .as_slice(),
        );
        let line = read_bounded_line(&mut reader, super::MAX_REQUEST_BYTES).unwrap();
        assert_eq!(
            line,
            br#"{"operation":"list_emails"}
"#
        );
    }

    #[test]
    fn upstream_auth_and_rate_limit_failures_map_to_stable_broker_codes() {
        let unauthorized = super::map_gmail_error(&anyhow::Error::new(GmailApiError::for_test(
            StatusCode::UNAUTHORIZED,
            "token rejected",
        )));
        let limited = super::map_gmail_error(&anyhow::Error::new(GmailApiError::for_test(
            StatusCode::TOO_MANY_REQUESTS,
            "slow down",
        )));
        assert!(matches!(
            unauthorized,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::ReauthenticationRequired,
                ..
            }
        ));
        assert!(matches!(
            limited,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::GmailRateLimited,
                ..
            }
        ));
    }

    #[test]
    fn gmail_http_and_transport_failures_remain_gmail_unavailable() {
        for error in [
            anyhow::Error::new(GmailApiError::for_test(
                StatusCode::FORBIDDEN,
                "private provider response detail",
            )),
            anyhow::anyhow!("upstream connection timed out"),
        ] {
            let response = super::map_gmail_error(&error);
            let encoded = serde_json::to_string(&response).unwrap();
            assert!(matches!(
                response,
                crate::mcp::BrokerResponse::Error {
                    code: crate::mcp::BrokerErrorCode::GmailUnavailable,
                    ..
                }
            ));
            assert!(!encoded.contains("private provider response detail"));
            assert!(!encoded.contains("upstream connection timed out"));
        }
    }

    #[test]
    fn credential_failures_are_separate_from_gmail_failures() {
        let unavailable = super::map_credential_error(&anyhow::anyhow!("OpenBao is sealed"));
        let missing = super::map_credential_error(&anyhow::anyhow!(
            "no stored Google refresh token; reauthenticate the selected MCP target"
        ));
        let invalid_grant = super::map_credential_error(&anyhow::Error::new(
            GoogleTokenError::for_test(
                StatusCode::BAD_REQUEST,
                Some("invalid_grant"),
                "revoked refresh token",
            ),
        ));
        let unavailable_text = serde_json::to_string(&unavailable).unwrap();

        assert!(matches!(
            &unavailable,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::CredentialUnavailable,
                message,
            } if message == "the selected account credential is temporarily unavailable"
        ));
        assert!(matches!(
            missing,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::ReauthenticationRequired,
                ..
            }
        ));
        assert!(matches!(
            invalid_grant,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::ReauthenticationRequired,
                ..
            }
        ));
        assert!(!unavailable_text.contains("sealed"));
        assert!(!unavailable_text.contains("refresh_token"));
    }

    #[cfg(unix)]
    #[test]
    fn socket_setup_refuses_an_existing_path() {
        let path = std::env::temp_dir().join(format!(
            "arqen-broker-existing-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"not a socket").unwrap();
        let error = prepare_socket_path(&path).unwrap_err();
        assert!(error.to_string().contains("already exists"));
        let _ = std::fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn socket_setup_reclaims_a_stale_unix_socket() {
        use std::os::unix::net::UnixListener;

        let path = std::env::temp_dir().join(format!(
            "arqen-broker-stale-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        prepare_socket_path(&path).unwrap();
        assert!(!path.exists());
    }
}

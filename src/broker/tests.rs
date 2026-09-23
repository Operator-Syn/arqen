#[cfg(test)]
mod tests {
    use super::{read_bounded_line, selected_target_account, target_ineligibility};
    #[cfg(unix)]
    use super::prepare_socket_path;
    use crate::{
        Account, AccountStore, ConnectionState, GMAIL_READONLY_SCOPE,
        auth::GoogleTokenError,
        gmail::{GmailApiError, ReadEmailTooLarge},
    };
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
    fn read_email_resolves_only_the_persisted_mcp_target_account() {
        let store = AccountStore::in_memory().unwrap();
        for (subject, email) in [
            ("selected-subject", "selected@example.com"),
            ("other-subject", "other@example.com"),
        ] {
            store
                .upsert_google_account(&Account {
                    id: format!("account-{subject}"),
                    subject: subject.into(),
                    email: email.into(),
                    display_name: None,
                    token_key: Some(format!("keyring:arqen:{subject}")),
                    granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into()]),
                    connection_state: ConnectionState::Connected,
                })
                .unwrap();
        }
        store
            .set_mcp_target_subject(Some("selected-subject"))
            .unwrap();

        let selected = selected_target_account(&store).unwrap();

        assert_eq!(selected.subject, "selected-subject");
        assert_eq!(selected.email, "selected@example.com");
    }

    #[test]
    fn read_email_account_store_failures_use_the_stable_internal_code() {
        let state = super::BrokerState {
            database_path: std::env::temp_dir()
                .join(format!("arqen-missing-parent-{}", uuid::Uuid::new_v4()))
                .join("accounts.sqlite3"),
            credentials_path: std::path::PathBuf::from("unused"),
            access_tokens: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        };
        let response = super::handle_read_email(
            crate::gmail::ReadEmailRequest {
                message_id: "message-123".into(),
            },
            &state,
        );
        assert!(matches!(
            response,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::Internal,
                ..
            }
        ));
    }

    #[test]
    fn list_labels_account_store_failures_use_the_stable_internal_code() {
        let state = super::BrokerState {
            database_path: std::env::temp_dir()
                .join(format!("arqen-missing-parent-{}", uuid::Uuid::new_v4()))
                .join("accounts.sqlite3"),
            credentials_path: std::path::PathBuf::from("unused"),
            access_tokens: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        };
        let response = super::handle_list_labels(&state);
        assert!(matches!(
            response,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::Internal,
                ..
            }
        ));
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
    fn list_labels_provider_and_credential_errors_keep_stable_public_codes() {
        use crate::mcp::{BrokerErrorCode, BrokerResponse};

        for (status, expected_code) in [
            (StatusCode::UNAUTHORIZED, BrokerErrorCode::ReauthenticationRequired),
            (StatusCode::TOO_MANY_REQUESTS, BrokerErrorCode::GmailRateLimited),
            (StatusCode::BAD_GATEWAY, BrokerErrorCode::GmailUnavailable),
        ] {
            let error = anyhow::Error::new(GmailApiError::for_test(
                status,
                "private Gmail response body",
            ));
            let response = super::map_gmail_error(&error);
            assert!(matches!(
                response,
                BrokerResponse::Error { code, .. } if code == expected_code
            ));
            assert!(!serde_json::to_string(&response)
                .unwrap()
                .contains("private Gmail response body"));
        }

        for (error, expected_code) in [
            (
                anyhow::anyhow!("protected credential service unavailable"),
                BrokerErrorCode::CredentialUnavailable,
            ),
            (
                anyhow::anyhow!("no stored Google refresh token"),
                BrokerErrorCode::ReauthenticationRequired,
            ),
        ] {
            let response = super::map_credential_error(&error);
            assert!(matches!(
                response,
                BrokerResponse::Error { code, .. } if code == expected_code
            ));
            let encoded = serde_json::to_string(&response).unwrap();
            assert!(!encoded.contains("protected credential service"));
            assert!(!encoded.contains("refresh token"));
        }
    }

    #[test]
    fn read_email_failures_map_to_stable_codes_without_provider_details() {
        use crate::mcp::{BrokerErrorCode, BrokerResponse};

        let cases = [
            (
                anyhow::Error::new(GmailApiError::for_test(
                    StatusCode::BAD_REQUEST,
                    "private invalid id detail",
                )),
                BrokerErrorCode::InvalidRequest,
            ),
            (
                anyhow::Error::new(GmailApiError::for_test(
                    StatusCode::NOT_FOUND,
                    "private missing message detail",
                )),
                BrokerErrorCode::MessageNotFound,
            ),
            (
                anyhow::Error::new(GmailApiError::for_test(
                    StatusCode::UNAUTHORIZED,
                    "private authentication detail",
                )),
                BrokerErrorCode::ReauthenticationRequired,
            ),
            (
                anyhow::Error::new(GmailApiError::for_test(
                    StatusCode::TOO_MANY_REQUESTS,
                    "private rate limit detail",
                )),
                BrokerErrorCode::GmailRateLimited,
            ),
            (
                anyhow::Error::new(GmailApiError::for_test(
                    StatusCode::BAD_GATEWAY,
                    "private provider detail",
                )),
                BrokerErrorCode::GmailUnavailable,
            ),
            (
                anyhow::Error::new(ReadEmailTooLarge),
                BrokerErrorCode::MessageTooLarge,
            ),
            (
                anyhow::anyhow!("private transport detail"),
                BrokerErrorCode::GmailUnavailable,
            ),
        ];

        for (error, expected_code) in cases {
            let response = super::map_read_email_error(&error);
            assert!(matches!(
                response,
                BrokerResponse::Error { code, .. } if code == expected_code
            ));
            let encoded = serde_json::to_value(&response).unwrap();
            assert_eq!(encoded["code"], expected_code.as_str());
            let encoded = encoded.to_string();
            assert!(!encoded.contains("private"));
            assert!(!encoded.contains("transport detail"));
        }
    }

    #[test]
    fn valid_message_id_with_gmail_not_found_maps_to_message_not_found() {
        use crate::mcp::{BrokerErrorCode, BrokerResponse};

        let request = crate::gmail::ReadEmailRequest {
            message_id: "18abc_123-ef".into(),
        }
        .validate()
        .expect("the test message ID is syntactically valid");
        let response = super::map_read_email_error(&anyhow::Error::new(
            GmailApiError::for_test(
                StatusCode::NOT_FOUND,
                "private Gmail response detail",
            ),
        ));

        assert!(matches!(
            &response,
            BrokerResponse::Error {
                code: BrokerErrorCode::MessageNotFound,
                message,
            } if message == "Message not found in the currently selected account. Use an ID returned by list_emails."
        ));
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(!encoded.contains("private Gmail response detail"));
        assert_ne!(BrokerErrorCode::MessageNotFound, BrokerErrorCode::InvalidMessageId);
        assert_ne!(BrokerErrorCode::MessageNotFound, BrokerErrorCode::Internal);
        assert_eq!(request.message_id, "18abc_123-ef");
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

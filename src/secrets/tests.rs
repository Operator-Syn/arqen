#[cfg(test)]
mod tests {
    use super::{OPENBAO_REFERENCE_PREFIX, OpenBaoClient, OpenBaoFailure, validate_path};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn mock_openbao(responses: Vec<(u16, &'static str)>) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let reason = match status {
                    200 => "OK",
                    204 => "No Content",
                    403 => "Forbidden",
                    404 => "Not Found",
                    _ => "Error",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        address
    }

    #[test]
    fn openbao_references_are_opaque_paths() {
        assert_eq!(
            format!("{OPENBAO_REFERENCE_PREFIX}secret/arqen/google/subject"),
            "openbao:secret/arqen/google/subject"
        );
        assert!(validate_path("secret/arqen/google/subject").is_ok());
        assert!(validate_path("secret/arqen/google/subject with spaces").is_err());
        assert!(validate_path("secret//subject").is_err());
    }

    #[test]
    fn openbao_client_round_trips_kv_operations() {
        let address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (204, "{}"),
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (
                200,
                r#"{"data":{"data":{"refresh_token":"refresh-value"}}}"#,
            ),
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (204, "{}"),
        ]);
        let client = OpenBaoClient::for_test(address);
        client
            .put(
                Some("openbao:arqen/google/subject"),
                "subject",
                "refresh-value",
            )
            .unwrap();
        assert_eq!(
            client
                .get(Some("openbao:arqen/google/subject"), "subject")
                .unwrap(),
            "refresh-value"
        );
        client
            .delete(Some("openbao:arqen/google/subject"), "subject")
            .unwrap();
    }

    #[test]
    fn openbao_client_maps_missing_and_forbidden_secrets() {
        let missing_address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (404, "{}"),
        ]);
        let missing = OpenBaoClient::for_test(missing_address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("no stored Google refresh token")
        );

        let forbidden_address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (403, "{}"),
        ]);
        let forbidden = OpenBaoClient::for_test(forbidden_address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            forbidden.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::CredentialReadFailed(403))
        );
    }

    #[test]
    fn openbao_client_rejects_malformed_auth_response() {
        let address = mock_openbao(vec![(200, "not-json")]);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::InvalidResponse)
        );
    }

    #[test]
    fn openbao_client_classifies_rejected_approle_without_provider_body() {
        let address = mock_openbao(vec![(400, r#"{"errors":["role_id=private-detail"]}"#)]);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::AppRoleRejected)
        );
        assert!(!error.to_string().contains("private-detail"));
    }

    #[test]
    fn openbao_client_keeps_other_login_statuses_separate_from_approle_rejection() {
        let address = mock_openbao(vec![(403, r#"{"errors":["private provider body"]}"#)]);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::LoginFailed(403))
        );
        assert!(!error.to_string().contains("private provider body"));
    }

    #[test]
    fn openbao_server_errors_map_to_unavailable() {
        let address = mock_openbao(vec![(503, r#"{"errors":["private provider body"]}"#)]);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::Unavailable)
        );
        assert!(!error.to_string().contains("private provider body"));
    }

    #[test]
    fn openbao_client_classifies_unavailable_service_without_transport_details() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::Unavailable)
        );
        assert!(!error.to_string().contains("Connection refused"));
    }

    #[test]
    fn openbao_client_classifies_credential_write_failures() {
        let address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"test-only"}}"#),
            (403, r#"{"errors":["private provider body"]}"#),
        ]);
        let error = OpenBaoClient::for_test(address)
            .put(
                Some("openbao:arqen/google/subject"),
                "subject",
                "test-only-refresh-token",
            )
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<OpenBaoFailure>(),
            Some(&OpenBaoFailure::CredentialWriteFailed(403))
        );
        assert!(!error.to_string().contains("private provider body"));
    }
}

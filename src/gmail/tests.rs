#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAX_RESULTS, GmailApi, ListEmailsRequest, MAX_MAX_RESULTS, truncate_snippet,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn request_defaults_to_inbox_and_bounded_page_size() {
        let request = ListEmailsRequest::default();
        assert_eq!(request.effective_query(), "in:inbox");
        assert_eq!(request.max_results, DEFAULT_MAX_RESULTS);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn request_rejects_invalid_limits_and_control_characters() {
        let mut request = ListEmailsRequest {
            max_results: MAX_MAX_RESULTS + 1,
            ..Default::default()
        };
        assert!(request.clone().validate().is_err());
        request.max_results = 1;
        request.query = Some("from:test\n".into());
        assert!(request.validate().is_err());
    }

    #[test]
    fn snippet_truncation_is_unicode_safe_and_reports_overflow() {
        let snippet = "界".repeat(301);
        let (truncated, was_truncated) = truncate_snippet(&snippet);
        assert_eq!(truncated.chars().count(), 300);
        assert!(was_truncated);
    }

    #[test]
    fn lists_messages_with_bounded_metadata_requests() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let server = thread::spawn(move || {
            let responses = [
                r#"{"messages":[{"id":"message-1","threadId":"thread-1"}],"nextPageToken":"next","resultSizeEstimate":1}"#,
                r#"{"id":"message-1","threadId":"thread-1","labelIds":["INBOX"],"snippet":"A short message","payload":{"headers":[{"name":"subject","value":"Hello"},{"name":"FROM","value":"sender@example.com"},{"name":"Date","value":"Mon, 1 Jan 2024 00:00:00 +0000"}]}}"#,
            ];
            for body in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });

        let api = GmailApi::with_base_url(&format!("http://{address}/")).unwrap();
        let result = api
            .list_emails(
                "test-access-token",
                "target@example.com",
                ListEmailsRequest {
                    query: Some(" from:sender@example.com ".into()),
                    label_ids: vec!["INBOX".into(), "IMPORTANT".into()],
                    max_results: 5,
                    page_token: Some("page-1".into()),
                    include_spam_trash: true,
                },
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(result.target_email, "target@example.com");
        assert_eq!(result.next_page_token.as_deref(), Some("next"));
        assert_eq!(result.messages[0].subject.as_deref(), Some("Hello"));
        assert_eq!(
            result.messages[0].from.as_deref(),
            Some("sender@example.com")
        );
        assert_eq!(result.messages[0].labels, vec!["INBOX".to_owned()]);

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /users/me/messages?"));
        assert!(requests[0].contains("q=from%3Asender%40example.com"));
        assert!(requests[0].contains("maxResults=5"));
        assert!(requests[0].contains("includeSpamTrash=true"));
        assert!(requests[0].contains("pageToken=page-1"));
        assert!(requests[0].contains("labelIds=INBOX"));
        assert!(requests[0].contains("labelIds=IMPORTANT"));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer test-access-token")
        );
        assert!(
            requests[1].starts_with("GET /users/me/messages/message-1?"),
            "captured requests: {requests:?}"
        );
        assert!(requests[1].contains("format=metadata"));
        assert!(requests[1].contains("metadataHeaders=From"));
        assert!(requests[1].contains("metadataHeaders=Subject"));
        assert!(requests[1].contains("metadataHeaders=Date"));
    }

    #[test]
    fn maps_upstream_failures_without_echoing_credentials() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"{"error":{"message":"token rejected"}}"#;
            write!(
                stream,
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let api = GmailApi::with_base_url(&format!("http://{address}/")).unwrap();
        let error = api
            .list_emails(
                "secret-access-token",
                "target@example.com",
                Default::default(),
            )
            .unwrap_err();
        server.join().unwrap();
        assert!(super::is_unauthorized(&error));
        assert!(error.to_string().contains("token rejected"));
        assert!(!error.to_string().contains("secret-access-token"));
    }
}

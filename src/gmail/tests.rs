#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAX_RESULTS, EmailBodyStatus, EmailLabelType, GmailApi, ListEmailsRequest,
        MAX_MAX_RESULTS,
        MAX_READ_EMAIL_BODY_BYTES, MessagePayload, MessageResource,
        ReadEmailRequest, ReadEmailTooLarge, email_read_response, read_body_text, truncate_snippet,
    };
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::{Value, json};
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
    fn request_accepts_page_size_boundaries_and_rejects_values_outside_them() {
        for max_results in [1, MAX_MAX_RESULTS] {
            assert!(ListEmailsRequest {
                max_results,
                ..Default::default()
            }
            .validate()
            .is_ok());
        }
        for max_results in [0, MAX_MAX_RESULTS + 1] {
            let error = ListEmailsRequest {
                max_results,
                ..Default::default()
            }
            .validate()
            .unwrap_err();
            assert_eq!(error.to_string(), "max_results must be between 1 and 50");
        }
    }

    #[test]
    fn request_rejects_control_characters() {
        let mut request = ListEmailsRequest {
            max_results: 1,
            ..Default::default()
        };
        request.query = Some("from:test\n".into());
        assert!(request.validate().is_err());
    }

    #[test]
    fn message_id_validation_rejects_malformed_values() {
        let oversized_id = "x".repeat(257);
        for message_id in ["", "has/slash", "has space", "ümlaut", &oversized_id] {
            assert!(ReadEmailRequest {
                message_id: message_id.into()
            }
            .validate()
            .is_err());
        }
        assert!(ReadEmailRequest {
            message_id: "18abc_123-ef".into()
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn syntactically_valid_missing_message_preserves_gmail_not_found_status() {
        let (base_url, server) = mock_gmail_response(
            r#"{"error":{"code":404,"message":"private provider detail"}}"#.into(),
            "404 Not Found",
        );
        let api = GmailApi::with_base_url(&base_url).unwrap();

        let error = api
            .read_email(
                "test-access-token",
                ReadEmailRequest {
                    message_id: "18abc_123-ef".into(),
                },
            )
            .unwrap_err();
        let request = server.join().unwrap();

        assert!(request.starts_with("GET /users/me/messages/18abc_123-ef?format=full&fields="));
        assert_eq!(
            error
                .downcast_ref::<super::GmailApiError>()
                .unwrap()
                .status(),
            reqwest::StatusCode::NOT_FOUND
        );
        assert!(!error.to_string().contains("private provider detail"));
    }

    #[test]
    fn list_labels_returns_system_and_custom_labels_with_opaque_ids() {
        let (base_url, server) = mock_gmail_response(
            r#"{"labels":[{"id":"INBOX","name":"Inbox","type":"system"},{"id":"Label_7","name":"Project Atlas","type":"user"}]}"#.into(),
            "200 OK",
        );
        let api = GmailApi::with_base_url(&base_url).unwrap();

        let result = api.list_labels("test-access-token").unwrap();
        let request = server.join().unwrap();

        assert!(request.starts_with("GET /users/me/labels?fields=labels%28id%2Cname%2Ctype%29"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-access-token"));
        assert_eq!(result.labels.len(), 2);
        assert_eq!(result.labels[0].id, "INBOX");
        assert_eq!(result.labels[0].name, "Inbox");
        assert_eq!(result.labels[0].label_type, EmailLabelType::System);
        assert_eq!(
            serde_json::to_value(&result).unwrap()["labels"][0]["type"],
            "system"
        );
        assert_eq!(result.labels[1].id, "Label_7");
        assert_eq!(result.labels[1].name, "Project Atlas");
        assert_eq!(result.labels[1].label_type, EmailLabelType::User);
        assert_eq!(
            serde_json::to_value(&result).unwrap()["labels"][1],
            json!({"id":"Label_7", "name":"Project Atlas", "type":"user"})
        );
    }

    #[test]
    fn label_id_can_be_passed_unchanged_to_filter_list_emails() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let responses = [
                (
                    r#"{"labels":[{"id":"Label_7","name":"Project Atlas","type":"user"}]}"#,
                    "200 OK",
                ),
                (
                    r#"{"messages":[{"id":"message-7","threadId":"thread-7"}],"resultSizeEstimate":1}"#,
                    "200 OK",
                ),
                (
                    r#"{"id":"message-7","threadId":"thread-7","labelIds":["Label_7"],"snippet":"Filtered result","payload":{"headers":[]}}"#,
                    "200 OK",
                ),
            ];
            let mut captured = Vec::new();
            for (body, status) in responses {
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
                captured.push(String::from_utf8(request).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            captured
        });
        let api = GmailApi::with_base_url(&format!("http://{address}/")).unwrap();

        let labels = api.list_labels("test-access-token").unwrap();
        let selected_id = labels
            .labels
            .iter()
            .find(|label| label.name == "Project Atlas")
            .unwrap()
            .id
            .clone();
        let result = api
            .list_emails(
                "test-access-token",
                "selected@example.com",
                ListEmailsRequest {
                    label_ids: vec![selected_id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();
        let requests = server.join().unwrap();

        assert_eq!(selected_id, "Label_7");
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].labels, vec!["Label_7"]);
        assert!(requests[0].starts_with("GET /users/me/labels?"));
        assert!(requests[1].starts_with("GET /users/me/messages?"));
        assert!(requests[1].contains("labelIds=Label_7"));
        assert!(!requests[1].contains("Project%20Atlas"));
        assert!(requests[2].starts_with("GET /users/me/messages/message-7?"));
    }

    #[test]
    fn reads_selected_message_with_nested_mime_and_prefers_plain_text() {
        let plain = URL_SAFE_NO_PAD.encode("The plain message body.");
        let html = URL_SAFE_NO_PAD.encode("<p>The <b>HTML alternative</b>.</p>");
        let attachment = URL_SAFE_NO_PAD.encode("Do not use attachment text.");
        let body = json!({
            "id": "message-123",
            "threadId": "thread-456",
            "labelIds": ["INBOX", "IMPORTANT"],
            "payload": {
                "mimeType": "multipart/mixed",
                "headers": [
                    {"name":"From", "value":"Sender <sender@example.com>"},
                    {"name":"To", "value":"Recipient <to@example.com>"},
                    {"name":"Cc", "value":"copy@example.com"},
                    {"name":"Bcc", "value":"blind@example.com"},
                    {"name":"Date", "value":"Mon, 1 Jan 2024 00:00:00 +0000"},
                    {"name":"Subject", "value":"A subject"}
                ],
                "parts": [
                    {
                        "mimeType":"multipart/alternative",
                        "parts":[
                            {"mimeType":"text/html", "body":{"data":html}},
                            {"mimeType":"text/plain", "body":{"data":plain}}
                        ]
                    },
                    {
                        "mimeType":"text/plain",
                        "filename":"notes.txt",
                        "body":{"data":attachment}
                    }
                ]
            }
        });
        let (base_url, server) = mock_gmail_response(body.to_string(), "200 OK");
        let api = GmailApi::with_base_url(&base_url).unwrap();

        let result = api
            .read_email(
                "test-access-token",
                ReadEmailRequest {
                    message_id: "message-123".into(),
                },
            )
            .unwrap();
        let request = server.join().unwrap();

        assert!(request.starts_with("GET /users/me/messages/message-123?format=full&fields="));
        assert!(request.to_ascii_lowercase().contains("authorization: bearer test-access-token"));
        assert_eq!(result.message_id, "message-123");
        assert_eq!(result.thread_id, "thread-456");
        assert_eq!(result.from.as_deref(), Some("Sender <sender@example.com>"));
        assert_eq!(result.recipients.to, vec!["Recipient <to@example.com>"]);
        assert_eq!(result.recipients.cc, vec!["copy@example.com"]);
        assert_eq!(result.recipients.bcc, vec!["blind@example.com"]);
        assert_eq!(result.date.as_deref(), Some("Mon, 1 Jan 2024 00:00:00 +0000"));
        assert_eq!(result.subject.as_deref(), Some("A subject"));
        assert_eq!(result.labels, vec!["INBOX", "IMPORTANT"]);
        assert_eq!(result.body_text.as_deref(), Some("The plain message body."));
        assert_eq!(result.body_status, EmailBodyStatus::Complete);
    }

    #[test]
    fn html_only_messages_are_converted_to_readable_text() {
        let html = URL_SAFE_NO_PAD.encode(
            "<html><body><h1>HTML-only body</h1><p>A &amp; B</p></body></html>",
        );
        let message = message_with_payload(json!({
            "mimeType":"text/html",
            "body":{"data":html}
        }));

        let result = email_read_response(message).unwrap();

        let body = result.body_text.unwrap();
        assert!(body.contains("HTML-only body"));
        assert!(body.contains("A & B"));
        assert!(!body.contains("<p>"));
        assert_eq!(result.body_status, EmailBodyStatus::Complete);
    }

    #[test]
    fn missing_and_malformed_body_parts_have_distinct_statuses() {
        let missing = email_read_response(message_with_payload(json!({
            "mimeType":"text/plain",
            "body":{"size":0}
        })))
        .unwrap();
        assert_eq!(missing.body_text, None);
        assert_eq!(missing.body_status, EmailBodyStatus::NoReadableBody);

        let malformed = email_read_response(message_with_payload(json!({
            "mimeType":"text/plain",
            "body":{"data":"%%%not-base64%%%", "size":4}
        })))
        .unwrap();
        assert_eq!(malformed.body_text, None);
        assert_eq!(malformed.body_status, EmailBodyStatus::Incomplete);
    }

    #[test]
    fn full_bodies_are_returned_at_the_limit_and_oversized_bodies_fail_clearly() {
        for (size, should_fit) in [
            (MAX_READ_EMAIL_BODY_BYTES, true),
            (MAX_READ_EMAIL_BODY_BYTES + 1, false),
        ] {
            let data = URL_SAFE_NO_PAD.encode("x".repeat(size));
            let payload: MessagePayload = serde_json::from_value(json!({
                "mimeType":"text/plain",
                "body":{"data":data, "size":size}
            }))
            .unwrap();
            let result = read_body_text(&payload);
            if should_fit {
                let (text, status) = result.unwrap();
                assert_eq!(text.unwrap().len(), size);
                assert_eq!(status, EmailBodyStatus::Complete);
            } else {
                assert!(result.unwrap_err().downcast_ref::<ReadEmailTooLarge>().is_some());
            }
        }
    }

    #[test]
    fn gmail_message_response_is_bounded_before_json_parsing() {
        let (base_url, server) = mock_gmail_response(
            format!("{{\"oversized\":\"{}\"}}", "x".repeat(2 * 1024 * 1024)),
            "200 OK",
        );
        let api = GmailApi::with_base_url(&base_url).unwrap();
        let error = api
            .read_email(
                "test-access-token",
                ReadEmailRequest {
                    message_id: "message-123".into(),
                },
            )
            .unwrap_err();
        server.join().unwrap();
        assert!(error.downcast_ref::<ReadEmailTooLarge>().is_some());
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
        let serialized = serde_json::to_value(&result).unwrap();
        assert!(serialized["messages"][0].get("body_text").is_none());
        assert!(serialized["messages"][0].get("body_status").is_none());

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

    fn message_with_payload(payload: Value) -> MessageResource {
        serde_json::from_value(json!({
            "id":"message-123",
            "threadId":"thread-456",
            "labelIds":["INBOX"],
            "payload":payload
        }))
        .unwrap()
    }

    fn mock_gmail_response(
        body: String,
        status: &str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let server = thread::spawn(move || {
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
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            String::from_utf8(request).unwrap()
        });
        (format!("http://{address}/"), server)
    }
}

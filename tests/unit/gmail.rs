// SPDX-License-Identifier: MPL-2.0
use super::{
    DEFAULT_MAX_RESULTS, EmailBodyStatus, EmailLabelType, GmailApi, ListEmailsRequest,
    MAX_MAX_RESULTS, MAX_READ_EMAIL_BODY_BYTES, MessagePayload, MessageResource, ReadEmailRequest,
    ReadEmailTooLarge, email_read_response, read_body_text, truncate_snippet,
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
        assert!(
            ListEmailsRequest {
                max_results,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
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
        assert!(
            ReadEmailRequest {
                message_id: message_id.into()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        ReadEmailRequest {
            message_id: "18abc_123-ef".into()
        }
        .validate()
        .is_ok()
    );
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
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-access-token")
    );
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
fn create_label_sends_only_the_name_and_returns_the_provider_id() {
    let (base_url, server) = mock_gmail_response(
        r#"{"id":"Label_7","name":"Project Atlas","type":"user"}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .create_label(
            "test-access-token",
            crate::gmail::CreateLabelRequest {
                name: "Project Atlas".into(),
            },
        )
        .unwrap();
    let request = server.join().unwrap();

    assert!(request.starts_with("POST /users/me/labels?fields=id%2Cname%2Ctype"));
    assert!(request.contains("{\"name\":\"Project Atlas\"}"));
    assert_eq!(result.id, "Label_7");
    assert_eq!(result.name, "Project Atlas");
    assert_eq!(result.label_type, EmailLabelType::User);
}

#[test]
fn label_name_validation_rejects_blank_and_control_input_without_normalizing_names() {
    for name in ["", " \t", "bad\nlabel"] {
        assert!(
            crate::gmail::CreateLabelRequest { name: name.into() }
                .validate()
                .is_err()
        );
    }
    let valid = crate::gmail::CreateLabelRequest {
        name: "  Project Atlas  ".into(),
    }
    .validate()
    .unwrap();
    assert_eq!(valid.name, "  Project Atlas  ");
}

#[test]
fn delete_label_checks_type_then_deletes_the_exact_opaque_id() {
    let (base_url, server) = mock_gmail_responses(vec![
        (r#"{"id":"Label_7","type":"user"}"#.into(), "200 OK".into()),
        ("{}".into(), "200 OK".into()),
    ]);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .delete_label(
            "test-access-token",
            crate::gmail::DeleteLabelRequest {
                label_id: "Label_7".into(),
            },
        )
        .unwrap();
    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /users/me/labels/Label_7?fields=id%2Ctype"));
    assert!(requests[1].starts_with("DELETE /users/me/labels/Label_7 "));
    assert_eq!(result.label_id, "Label_7");
    assert!(result.deleted);
}

#[test]
fn delete_label_rejects_system_type_without_sending_delete() {
    let (base_url, server) =
        mock_gmail_response(r#"{"id":"INBOX","type":"system"}"#.into(), "200 OK");
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .delete_label(
            "test-access-token",
            crate::gmail::DeleteLabelRequest {
                label_id: "INBOX".into(),
            },
        )
        .unwrap_err();
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /users/me/labels/INBOX?fields=id%2Ctype"));
    assert!(error.downcast_ref::<super::SystemLabelError>().is_some());
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
fn marking_read_and_unread_changes_only_unread_and_is_idempotent() {
    let mut responses = Vec::new();
    for labels in [
        r#"["INBOX","STARRED"]"#,
        r#"["INBOX","STARRED"]"#,
        r#"["INBOX","STARRED","UNREAD"]"#,
        r#"["INBOX","STARRED","UNREAD"]"#,
    ] {
        responses.push((
            format!(r#"{{"id":"message-123","labelIds":{labels}}}"#),
            "200 OK".into(),
        ));
        responses.push((
            format!(r#"{{"id":"message-123","labelIds":{labels}}}"#),
            "200 OK".into(),
        ));
    }
    let (base_url, server) = mock_gmail_responses(responses);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let request = || ReadEmailRequest {
        message_id: "message-123".into(),
    };

    let read = api.mark_email_read("test-access-token", request()).unwrap();
    let read_again = api.mark_email_read("test-access-token", request()).unwrap();
    let unread = api
        .mark_email_unread("test-access-token", request())
        .unwrap();
    let unread_again = api
        .mark_email_unread("test-access-token", request())
        .unwrap();
    let requests = server.join().unwrap();

    assert_eq!(
        read,
        super::EmailReadState {
            message_id: "message-123".into(),
            is_read: true,
        }
    );
    assert_eq!(read_again, read);
    assert_eq!(
        unread,
        super::EmailReadState {
            message_id: "message-123".into(),
            is_read: false,
        }
    );
    assert_eq!(unread_again, unread);

    for request in requests.iter().step_by(2) {
        assert!(
            request.starts_with(
                "GET /users/me/messages/message-123?format=minimal&fields=id%2ClabelIds"
            )
        );
    }
    for request in requests.iter().skip(1).step_by(2) {
        assert!(request.starts_with(
            "POST /users/me/messages/message-123/modify?fields=id%2ClabelIds HTTP/1.1"
        ));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-access-token")
        );
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert!(body.starts_with('{'));
    }
    for request in requests.iter().skip(1).step_by(2).take(2) {
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "removeLabelIds": ["UNREAD"]
            })
        );
    }
    for request in requests.iter().skip(5).step_by(2) {
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({
                "addLabelIds": ["UNREAD"]
            })
        );
    }
}

#[test]
fn create_draft_encodes_required_headers_and_returns_distinct_ids() {
    let (base_url, server) = mock_gmail_response(
        r#"{"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"]}}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .create_draft(
            "token",
            crate::gmail::CreateDraftRequest {
                to: "person@example.com".into(),
                subject: "Hello".into(),
                body: "Body text".into(),
            },
        )
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /users/me/drafts HTTP/1.1"));
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    let payload: Value = serde_json::from_str(body).unwrap();
    let raw = URL_SAFE_NO_PAD
        .decode(payload["message"]["raw"].as_str().unwrap())
        .unwrap();
    let raw = String::from_utf8(raw).unwrap();
    assert!(raw.contains("To: person@example.com\r\n"));
    assert!(raw.contains("Subject: Hello\r\n"));
    assert!(raw.ends_with("\r\n\r\nBody text"));
    assert_eq!(result.draft_id, "draft-1");
    assert_eq!(result.message_id, "message-1");
}

#[test]
fn create_reply_draft_uses_source_reply_headers_and_thread() {
    let (base_url, server) = mock_gmail_responses(vec![
        (r#"{"id":"source-1","threadId":"thread-1","payload":{"headers":[{"name":"From","value":"sender@example.com"},{"name":"Subject","value":"Question"},{"name":"Message-ID","value":"<original@example.com>"}]}}"#.into(), "200 OK".into()),
        (r#"{"id":"draft-1","message":{"id":"draft-message-1","threadId":"thread-1","labelIds":["DRAFT"]}}"#.into(), "200 OK".into()),
    ]);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .create_reply_draft(
            "token",
            crate::gmail::CreateReplyDraftRequest {
                message_id: "source-1".into(),
                body: "A reply".into(),
            },
        )
        .unwrap();
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /users/me/messages/source-1?format=full&fields="));
    assert!(requests[1].starts_with("POST /users/me/drafts HTTP/1.1"));
    let (_, body) = requests[1].split_once("\r\n\r\n").unwrap();
    let payload: Value = serde_json::from_str(body).unwrap();
    assert_eq!(payload["message"]["threadId"], "thread-1");
    let raw = URL_SAFE_NO_PAD
        .decode(payload["message"]["raw"].as_str().unwrap())
        .unwrap();
    let raw = String::from_utf8(raw).unwrap();
    assert!(raw.contains("To: sender@example.com\r\n"));
    assert!(raw.contains("Subject: Re: Question\r\n"));
    assert!(raw.contains("In-Reply-To: <original@example.com>\r\n"));
    assert_eq!(result.draft_id, "draft-1");
}

#[test]
fn create_reply_draft_reports_missing_source_message() {
    let (base_url, server) = mock_gmail_response(
        r#"{"error":{"code":404,"message":"private provider detail"}}"#.into(),
        "404 Not Found",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .create_reply_draft(
            "token",
            crate::gmail::CreateReplyDraftRequest {
                message_id: "source-1".into(),
                body: "Reply".into(),
            },
        )
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<super::GmailApiError>()
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    assert!(
        server
            .join()
            .unwrap()
            .starts_with("GET /users/me/messages/source-1?")
    );
}

#[test]
fn list_drafts_returns_draft_and_message_ids_separately() {
    let (base_url, server) = mock_gmail_responses(vec![
        (r#"{"drafts":[{"id":"draft-1"}],"resultSizeEstimate":1}"#.into(), "200 OK".into()),
        (r#"{"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"],"snippet":"Draft preview","payload":{"headers":[{"name":"To","value":"person@example.com"},{"name":"Subject","value":"Hello"}]}}}"#.into(), "200 OK".into()),
    ]);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .list_drafts("token", "target@example.com", Default::default())
        .unwrap();
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /users/me/drafts?"));
    assert!(requests[1].starts_with("GET /users/me/drafts/draft-1?format=metadata&fields="));
    assert_eq!(result.drafts[0].draft_id, "draft-1");
    assert_eq!(result.drafts[0].message_id, "message-1");
    assert_eq!(result.drafts[0].to, vec!["person@example.com"]);
}

#[test]
fn draft_listing_rejects_detail_for_a_different_draft() {
    let (base_url, server) = mock_gmail_responses(vec![
        (json!({"drafts":[{"id":"draft-1"}]}).to_string(), "200 OK".into()),
        (json!({"id":"other-draft","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"]}}).to_string(), "200 OK".into()),
    ]);
    let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
        "token",
        "selected@example.com",
        Default::default(),
    );
    assert_eq!(server.join().unwrap().len(), 2);
    assert!(
        result.is_err(),
        "a mismatched draft must not become a listed item"
    );
}

#[test]
fn draft_listing_requires_the_draft_label_and_rejects_other_labels() {
    for labels in [
        None,
        Some(json!([])),
        Some(json!(["INBOX"])),
        Some(json!(["DRAFT", "INBOX"])),
    ] {
        let mut detail = json!({"id":"draft-1","message":{"id":"message-1","threadId":"thread-1"}});
        if let Some(labels) = labels {
            detail["message"]["labelIds"] = labels;
        }
        let (base_url, server) = mock_gmail_responses(vec![
            (
                json!({"drafts":[{"id":"draft-1"}]}).to_string(),
                "200 OK".into(),
            ),
            (detail.to_string(), "200 OK".into()),
        ]);
        let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
            "token",
            "selected@example.com",
            Default::default(),
        );
        assert_eq!(server.join().unwrap().len(), 2);
        assert!(result.is_err(), "metadata must affirm only the DRAFT label");
    }
}

#[test]
fn draft_listing_bounds_unicode_snippets_using_the_email_limit() {
    let snippet = "界".repeat(301);
    let (base_url, server) = mock_gmail_responses(vec![
        (json!({"drafts":[{"id":"draft-1"}]}).to_string(), "200 OK".into()),
        (json!({"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"],"snippet":snippet}}).to_string(), "200 OK".into()),
    ]);
    let result = GmailApi::with_base_url(&base_url)
        .unwrap()
        .list_drafts("token", "selected@example.com", Default::default())
        .unwrap();
    assert_eq!(server.join().unwrap().len(), 2);
    assert_eq!(result.drafts[0].snippet, "界".repeat(300));
}

#[test]
fn draft_listing_rejects_invalid_references_before_fetching_details() {
    for id in ["", "bad/id", "bad id"] {
        let (base_url, server) =
            mock_gmail_response(json!({"drafts":[{"id":id}]}).to_string(), "200 OK");
        let error = GmailApi::with_base_url(&base_url)
            .unwrap()
            .list_drafts("token", "selected@example.com", Default::default())
            .unwrap_err();
        assert!(server.join().unwrap().starts_with("GET /users/me/drafts?"));
        let diagnostic = error.downcast_ref::<super::DraftListError>().unwrap();
        assert_eq!(diagnostic.stage, super::DraftListStage::List);
        assert_eq!(diagnostic.category, super::DraftListCategory::Validation);
        assert!(error.to_string().contains("no partial page"));
    }
}

#[test]
fn draft_listing_rejects_oversized_list_response() {
    let (base_url, server) = mock_gmail_response(
        json!({"drafts":[],"unexpected":"x".repeat(2 * 1024 * 1024)}).to_string(),
        "200 OK",
    );
    let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
        "token",
        "selected@example.com",
        Default::default(),
    );
    server.join().unwrap();
    assert!(
        result.is_err(),
        "the list response must be bounded before parsing"
    );
}

#[test]
fn draft_listing_accepts_empty_pages_without_detail_requests() {
    for body in ["{}", r#"{"drafts":[],"resultSizeEstimate":0}"#] {
        let (base_url, server) = mock_gmail_response(body.into(), "200 OK");
        let result = GmailApi::with_base_url(&base_url)
            .unwrap()
            .list_drafts("token", "selected@example.com", Default::default())
            .unwrap();
        assert!(server.join().unwrap().starts_with("GET /users/me/drafts?"));
        assert_eq!(result.target_email, "selected@example.com");
        assert!(result.drafts.is_empty());
        assert_eq!(result.next_page_token, None);
    }
}

#[test]
fn draft_listing_preserves_pagination_and_documented_metadata_projection() {
    let (base_url, server) = mock_gmail_responses(vec![
        (json!({"drafts":[{"id":"draft-1"}],"nextPageToken":"next+/=","resultSizeEstimate":7}).to_string(), "200 OK".into()),
        (json!({"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"],"payload":{"headers":[{"name":"tO","value":"person@example.com"},{"name":"SUBJECT","value":"Hello"},{"name":"Date","value":"today"}]}}}).to_string(), "200 OK".into()),
    ]);
    let result = GmailApi::with_base_url(&base_url)
        .unwrap()
        .list_drafts(
            "token",
            "selected@example.com",
            crate::gmail::ListDraftsRequest {
                max_results: 1,
                page_token: Some("page+/=".into()),
            },
        )
        .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2, "only one requested page is fetched");
    let query = |request: &str| {
        let path = request
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        reqwest::Url::parse(&format!("http://localhost{path}"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        query(&requests[0]),
        vec![
            ("maxResults".into(), "1".into()),
            (
                "fields".into(),
                "drafts(id),nextPageToken,resultSizeEstimate".into()
            ),
            ("pageToken".into(), "page+/=".into()),
        ]
    );
    assert_eq!(
        query(&requests[1]),
        vec![
            ("format".into(), "metadata".into()),
            (
                "fields".into(),
                "id,message(id,threadId,labelIds,snippet,payload(headers(name,value)))".into()
            ),
        ]
    );
    assert_eq!(result.next_page_token.as_deref(), Some("next+/="));
    assert_eq!(result.result_size_estimate, Some(7));
    assert_eq!(result.drafts[0].to, vec!["person@example.com"]);
    assert_eq!(result.drafts[0].subject.as_deref(), Some("Hello"));
    assert_eq!(result.drafts[0].date.as_deref(), Some("today"));
    assert_eq!(result.drafts[0].snippet, "");
}

#[test]
fn draft_listing_rejects_malformed_list_responses() {
    for body in ["not-json", "[]", r#"{"drafts":null}"#, r#"{"drafts":[{}]}"#] {
        let (base_url, server) = mock_gmail_response(body.into(), "200 OK");
        let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
            "token",
            "selected@example.com",
            Default::default(),
        );
        server.join().unwrap();
        assert!(result.is_err());
    }
}

#[test]
fn draft_listing_rejects_missing_or_empty_detail_identity() {
    for detail in [
        json!({}),
        json!({"id":"draft-1","message":{"id":"message-1","labelIds":["DRAFT"]}}),
        json!({"id":"draft-1","message":{"id":"","threadId":"thread-1","labelIds":["DRAFT"]}}),
        json!({"id":"draft-1","message":{"id":"message-1","threadId":"","labelIds":["DRAFT"]}}),
    ] {
        let (base_url, server) = mock_gmail_responses(vec![
            (
                json!({"drafts":[{"id":"draft-1"}]}).to_string(),
                "200 OK".into(),
            ),
            (detail.to_string(), "200 OK".into()),
        ]);
        let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
            "token",
            "selected@example.com",
            Default::default(),
        );
        assert_eq!(server.join().unwrap().len(), 2);
        assert!(result.is_err());
    }
}

#[test]
fn draft_listing_fails_the_page_if_a_draft_disappears_midpage() {
    let (base_url, server) = mock_gmail_responses(vec![
        (json!({"drafts":[{"id":"draft-1"},{"id":"draft-2"}],"nextPageToken":"next"}).to_string(), "200 OK".into()),
        (json!({"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"]}}).to_string(), "200 OK".into()),
        (r#"{"error":{"message":"private provider detail"}}"#.into(), "404 Not Found".into()),
    ]);
    let error = GmailApi::with_base_url(&base_url)
        .unwrap()
        .list_drafts(
            "secret-test-token",
            "selected@example.com",
            Default::default(),
        )
        .unwrap_err();
    assert_eq!(server.join().unwrap().len(), 3);
    assert_eq!(
        error
            .downcast_ref::<super::GmailApiError>()
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    assert!(!error.to_string().contains("private provider detail"));
    assert!(!error.to_string().contains("secret-test-token"));
}

#[test]
fn draft_listing_preserves_failed_list_and_detail_statuses_without_provider_bodies() {
    for (status, expected) in [
        ("401 Unauthorized", reqwest::StatusCode::UNAUTHORIZED),
        (
            "429 Too Many Requests",
            reqwest::StatusCode::TOO_MANY_REQUESTS,
        ),
        (
            "503 Service Unavailable",
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        for detail_failure in [false, true] {
            let mut responses = Vec::new();
            if detail_failure {
                responses.push((
                    json!({"drafts":[{"id":"draft-1"}]}).to_string(),
                    "200 OK".into(),
                ));
            }
            responses.push((
                r#"{"error":{"message":"private provider detail"}}"#.into(),
                status.into(),
            ));
            let (base_url, server) = mock_gmail_responses(responses);
            let error = GmailApi::with_base_url(&base_url)
                .unwrap()
                .list_drafts(
                    "secret-test-token",
                    "selected@example.com",
                    Default::default(),
                )
                .unwrap_err();
            assert_eq!(
                server.join().unwrap().len(),
                if detail_failure { 2 } else { 1 }
            );
            assert_eq!(
                error
                    .downcast_ref::<super::GmailApiError>()
                    .unwrap()
                    .status(),
                expected
            );
            assert!(!error.to_string().contains("private provider detail"));
            assert!(!error.to_string().contains("secret-test-token"));
        }
    }
}

#[test]
fn draft_listing_rejects_oversized_detail_response() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let base_url = format!("http://{}/", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for body in [
            json!({"drafts":[{"id":"draft-1"}]}).to_string(),
            json!({"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"]},"unexpected":"x".repeat(2 * 1024 * 1024)}).to_string(),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buffer[..read]);
            }
            // A bounded client can intentionally close before consuming this body.
            if let Err(error) = write!(stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len()) {
                assert!(matches!(error.kind(), std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset));
            }
        }
    });
    let result = GmailApi::with_base_url(&base_url).unwrap().list_drafts(
        "token",
        "selected@example.com",
        Default::default(),
    );
    server.join().unwrap();
    assert!(
        result.is_err(),
        "detail responses must also be bounded before parsing"
    );
}

#[test]
fn delete_draft_uses_drafts_resource_and_accepts_empty_success_response() {
    let (base_url, server) = mock_gmail_response(String::new(), "204 No Content");
    let api = GmailApi::with_base_url(&base_url).unwrap();
    api.delete_draft("token", "draft-1").unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("DELETE /users/me/drafts/draft-1 HTTP/1.1"));
}

#[test]
fn send_draft_uses_the_existing_draft_id_and_returns_sent_message_id() {
    let (base_url, server) = mock_gmail_response(
        r#"{"id":"sent-message-1","threadId":"thread-1"}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api.send_draft("token", "draft-1").unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /users/me/drafts/send HTTP/1.1"));
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(body).unwrap(),
        json!({"id":"draft-1"})
    );
    assert_eq!(result.draft_id, "draft-1");
    assert_eq!(result.message_id, "sent-message-1");
}

#[test]
fn draft_validation_rejects_header_injection_and_blank_fields() {
    for request in [
        crate::gmail::CreateDraftRequest {
            to: "not-an-email".into(),
            subject: "Hi".into(),
            body: "body".into(),
        },
        crate::gmail::CreateDraftRequest {
            to: "person@example.com\r\nBcc:other@example.com".into(),
            subject: "Hi".into(),
            body: "body".into(),
        },
        crate::gmail::CreateDraftRequest {
            to: "person@example.com".into(),
            subject: "  ".into(),
            body: "body".into(),
        },
        crate::gmail::CreateDraftRequest {
            to: "person@example.com".into(),
            subject: "Hello".into(),
            body: " \t ".into(),
        },
    ] {
        assert!(request.validate().is_err());
    }
    assert!(
        crate::gmail::CreateReplyDraftRequest {
            message_id: "source-1".into(),
            body: String::new(),
        }
        .validate()
        .is_err()
    );
    for body in ["body\0unsafe".to_owned(), "x".repeat(24_577)] {
        assert!(
            crate::gmail::CreateDraftRequest {
                to: "person@example.com".into(),
                subject: "Hello".into(),
                body,
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn label_and_read_state_mutations_reject_draft_messages_before_modifying_them() {
    let (base_url, server) = mock_gmail_response(
        r#"{"id":"draft-message","labelIds":["DRAFT"]}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .mark_email_read(
            "token",
            ReadEmailRequest {
                message_id: "draft-message".into(),
            },
        )
        .unwrap_err();
    assert!(super::is_draft_message_mutation(&error));
    let request = server.join().unwrap();
    assert!(
        request.starts_with(
            "GET /users/me/messages/draft-message?format=minimal&fields=id%2ClabelIds"
        )
    );

    let (base_url, server) = mock_gmail_response(
        r#"{"id":"draft-message","labelIds":["DRAFT"]}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .apply_label(
            "token",
            crate::gmail::ApplyLabelRequest {
                message_id: "draft-message".into(),
                label_id: "Label_1".into(),
            },
        )
        .unwrap_err();
    match error {
        crate::gmail::ApplyLabelFailure::MessageModify(error) => {
            assert!(super::is_draft_message_mutation(&error))
        }
        other => panic!("unexpected draft label result: {other:?}"),
    }
    let request = server.join().unwrap();
    assert!(
        request.starts_with(
            "GET /users/me/messages/draft-message?format=minimal&fields=id%2ClabelIds"
        )
    );
}

#[test]
fn message_trash_sends_explicit_zero_length_http11_body() {
    let (base_url, server) = mock_gmail_responses(vec![
        (
            r#"{"id":"message-123","labelIds":["INBOX"]}"#.into(),
            "200 OK".into(),
        ),
        (r#"{"id":"message-123"}"#.into(), "200 OK".into()),
    ]);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let result = api
        .trash_email(
            "test-access-token",
            ReadEmailRequest {
                message_id: "message-123".into(),
            },
        )
        .unwrap();
    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].lines().next().unwrap(),
        "GET /users/me/messages/message-123?format=minimal&fields=id%2ClabelIds HTTP/1.1"
    );
    let (headers, body) = requests[1].split_once("\r\n\r\n").unwrap();
    assert_eq!(
        headers.lines().next().unwrap(),
        "POST /users/me/messages/message-123/trash?fields=id HTTP/1.1"
    );
    let headers = headers.to_ascii_lowercase();
    assert!(
        headers.lines().any(|line| line == "content-length: 0"),
        "empty Trash POST must explicitly send Content-Length: 0"
    );
    assert!(
        !headers
            .lines()
            .any(|line| line.starts_with("transfer-encoding:"))
    );
    assert!(body.is_empty(), "Trash POST must not send a payload");
    assert_eq!(result.message_id, "message-123");
    assert!(result.trashed);
}

#[test]
fn message_trash_rejects_unexpected_response_message_id() {
    let (base_url, server) = mock_gmail_responses(vec![
        (
            r#"{"id":"message-123","labelIds":["INBOX"]}"#.into(),
            "200 OK".into(),
        ),
        (r#"{"id":"different-message"}"#.into(), "200 OK".into()),
    ]);
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .trash_email(
            "test-access-token",
            ReadEmailRequest {
                message_id: "message-123".into(),
            },
        )
        .unwrap_err();
    assert_eq!(server.join().unwrap().len(), 2);
    assert_eq!(
        error.to_string(),
        "Gmail trash response has an unexpected message ID"
    );
}

#[test]
fn message_trash_rejects_draft_messages_before_modifying_them() {
    let (base_url, server) = mock_gmail_response(
        r#"{"id":"draft-message","labelIds":["DRAFT"]}"#.into(),
        "200 OK",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .trash_email(
            "token",
            ReadEmailRequest {
                message_id: "draft-message".into(),
            },
        )
        .unwrap_err();
    assert!(super::is_draft_message_mutation(&error));
    let request = server.join().unwrap();
    assert!(
        request.starts_with(
            "GET /users/me/messages/draft-message?format=minimal&fields=id%2ClabelIds"
        )
    );
}

#[test]
fn mark_email_preserves_gmail_not_found_status_without_provider_details() {
    let (base_url, server) = mock_gmail_response(
        r#"{"error":{"code":404,"message":"private provider detail"}}"#.into(),
        "404 Not Found",
    );
    let api = GmailApi::with_base_url(&base_url).unwrap();
    let error = api
        .mark_email_read(
            "test-access-token",
            ReadEmailRequest {
                message_id: "valid-message-id".into(),
            },
        )
        .unwrap_err();
    let request = server.join().unwrap();

    assert!(request.starts_with(
        "GET /users/me/messages/valid-message-id?format=minimal&fields=id%2ClabelIds"
    ));
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
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-access-token")
    );
    assert_eq!(result.message_id, "message-123");
    assert_eq!(result.thread_id, "thread-456");
    assert_eq!(result.from.as_deref(), Some("Sender <sender@example.com>"));
    assert_eq!(result.recipients.to, vec!["Recipient <to@example.com>"]);
    assert_eq!(result.recipients.cc, vec!["copy@example.com"]);
    assert_eq!(result.recipients.bcc, vec!["blind@example.com"]);
    assert_eq!(
        result.date.as_deref(),
        Some("Mon, 1 Jan 2024 00:00:00 +0000")
    );
    assert_eq!(result.subject.as_deref(), Some("A subject"));
    assert_eq!(result.labels, vec!["INBOX", "IMPORTANT"]);
    assert_eq!(result.body_text.as_deref(), Some("The plain message body."));
    assert_eq!(result.body_status, EmailBodyStatus::Complete);
}

#[test]
fn html_only_messages_are_converted_to_readable_text() {
    let html =
        URL_SAFE_NO_PAD.encode("<html><body><h1>HTML-only body</h1><p>A &amp; B</p></body></html>");
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
            assert!(
                result
                    .unwrap_err()
                    .downcast_ref::<ReadEmailTooLarge>()
                    .is_some()
            );
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
    assert!(!error.to_string().contains("token rejected"));
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

fn mock_gmail_response(body: String, status: &str) -> (String, thread::JoinHandle<String>) {
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

fn mock_gmail_responses(
    responses: Vec<(String, String)>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut requests = Vec::with_capacity(responses.len());
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
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            requests.push(String::from_utf8(request).unwrap());
            let sent = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            // Bounded clients may close before the oversized fixture is sent.
            if let Err(error) = sent {
                assert!(
                    body.len() > 2 * 1024 * 1024
                        && matches!(
                            error.kind(),
                            std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                        ),
                    "fixture response write failed: {error}"
                );
            }
        }
        requests
    });
    (format!("http://{address}/"), server)
}

#[test]
fn priority_a_label_delete_accepts_empty_success() {
    for status in ["200 OK", "204 No Content"] {
        let (base_url, server) = mock_gmail_responses(vec![
            (r#"{"id":"Label_7","type":"user"}"#.into(), "200 OK".into()),
            (String::new(), status.into()),
        ]);
        let result = GmailApi::with_base_url(&base_url).unwrap().delete_label(
            "test-token",
            crate::gmail::DeleteLabelRequest {
                label_id: "Label_7".into(),
            },
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(result.unwrap().deleted);
    }
}

#[test]
fn priority_a_label_create_verifies_missing_type_by_exact_id() {
    let (base_url, server) = mock_gmail_responses(vec![
        (r#"{"id":"Label_7","name":"Atlas"}"#.into(), "200 OK".into()),
        (
            r#"{"id":"Label_7","name":"Atlas","type":"user"}"#.into(),
            "200 OK".into(),
        ),
    ]);
    let result = GmailApi::with_base_url(&base_url).unwrap().create_label(
        "test-token",
        crate::gmail::CreateLabelRequest {
            name: "Atlas".into(),
        },
    );
    assert!(result.is_ok(), "{result:?}");
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /users/me/labels?"));
    assert!(requests[1].starts_with("GET /users/me/labels/Label_7?"));
    assert!(requests.iter().all(|r| {
        r.to_ascii_lowercase()
            .contains("authorization: bearer test-token")
    }));
    assert_eq!(result.unwrap().id, "Label_7");
}

#[test]
fn priority_a_label_create_rejects_identity_mismatch_without_retry() {
    for body in [
        r#"{"id":"Label_7","name":"Wrong","type":"user"}"#,
        r#"{"id":"","name":"Atlas","type":"user"}"#,
        r#"{"id":"Label_7","name":"Atlas","type":"system"}"#,
        r#"{"id":"Label_7","name":"Atlas","type":"unknown"}"#,
        r#"{"id":"Label_7","name":"Atlas","type":null}"#,
        "not-json",
    ] {
        let (base_url, server) = mock_gmail_response(body.into(), "200 OK");
        let result = GmailApi::with_base_url(&base_url).unwrap().create_label(
            "test-token",
            crate::gmail::CreateLabelRequest {
                name: "Atlas".into(),
            },
        );
        server.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.to_string().contains("may have succeeded"), "{error}");
        assert!(!crate::gmail::is_unauthorized(&error));
    }
}

#[test]
fn priority_a_label_create_followup_failure_is_not_a_retryable_write() {
    for (body, status) in [
        (r#"{"id":"Other","name":"Atlas","type":"user"}"#, "200 OK"),
        (r#"{"id":"Label_7","name":"Wrong","type":"user"}"#, "200 OK"),
        (
            r#"{"id":"Label_7","name":"Atlas","type":"system"}"#,
            "200 OK",
        ),
        (r#"{"id":"Label_7","name":"Atlas"}"#, "200 OK"),
        ("{}", "401 Unauthorized"),
        ("{}", "404 Not Found"),
    ] {
        let (base_url, server) = mock_gmail_responses(vec![
            (r#"{"id":"Label_7","name":"Atlas"}"#.into(), "200 OK".into()),
            (body.into(), status.into()),
        ]);
        let result = GmailApi::with_base_url(&base_url).unwrap().create_label(
            "test-token",
            crate::gmail::CreateLabelRequest {
                name: "Atlas".into(),
            },
        );
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        let error = result.unwrap_err();
        assert!(error.to_string().contains("may have succeeded"), "{error}");
        assert!(
            !crate::gmail::is_unauthorized(&error),
            "a GET 401 must never retry the POST"
        );
    }
}

#[test]
fn priority_a_label_delete_invalid_success_is_uncertain() {
    for body in [
        "null",
        "[]",
        "true",
        "not-json",
        r#"{"unexpected":true}"#,
        " ",
    ] {
        let (base_url, server) = mock_gmail_responses(vec![
            (r#"{"id":"Label_7","type":"user"}"#.into(), "200 OK".into()),
            (body.into(), "200 OK".into()),
        ]);
        let result = GmailApi::with_base_url(&base_url).unwrap().delete_label(
            "test-token",
            crate::gmail::DeleteLabelRequest {
                label_id: "Label_7".into(),
            },
        );
        assert_eq!(server.join().unwrap().len(), 2);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("may have succeeded")
        );
    }
}

#[test]
fn priority_a_label_generic_response_is_bounded() {
    let body = format!(
        r#"{{"labels":[],"padding":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    for chunked in [false, true] {
        let (base_url, server) = priority_a_label_framed_response(body.clone(), chunked);
        let error = GmailApi::with_base_url(&base_url)
            .unwrap()
            .list_labels("test-token")
            .unwrap_err();
        server.join().unwrap();
        assert!(error.to_string().contains("response exceeds"), "{error}");
    }
}

fn priority_a_label_framed_response(
    body: String,
    chunked: bool,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
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
        if chunked {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                body.len()
            );
        } else {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (format!("http://{address}/"), server)
}

#[test]
fn priority_a_label_delete_response_is_bounded() {
    let body = format!("{}{{}}", " ".repeat(2 * 1024 * 1024));
    let (base_url, server) = mock_gmail_responses(vec![
        (r#"{"id":"Label_7","type":"user"}"#.into(), "200 OK".into()),
        (body, "200 OK".into()),
    ]);
    let result = GmailApi::with_base_url(&base_url).unwrap().delete_label(
        "test-token",
        crate::gmail::DeleteLabelRequest {
            label_id: "Label_7".into(),
        },
    );
    server.join().unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("may have succeeded")
    );
}

#[test]
fn priority_a_label_non_success_statuses_are_preserved() {
    for status in [
        "400 Bad Request",
        "401 Unauthorized",
        "404 Not Found",
        "409 Conflict",
        "429 Too Many Requests",
        "500 Internal Server Error",
    ] {
        let (base_url, server) = mock_gmail_response("not-json".into(), status);
        let error = GmailApi::with_base_url(&base_url)
            .unwrap()
            .create_label(
                "test-token",
                crate::gmail::CreateLabelRequest {
                    name: "Atlas".into(),
                },
            )
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(
            error
                .downcast_ref::<crate::gmail::GmailApiError>()
                .unwrap()
                .status()
                .as_u16()
                .to_string(),
            &status[..3]
        );
        let (base_url, server) = mock_gmail_responses(vec![
            (r#"{"id":"Label_7","type":"user"}"#.into(), "200 OK".into()),
            (String::new(), status.into()),
        ]);
        let error = GmailApi::with_base_url(&base_url)
            .unwrap()
            .delete_label(
                "test-token",
                crate::gmail::DeleteLabelRequest {
                    label_id: "Label_7".into(),
                },
            )
            .unwrap_err();
        server.join().unwrap();
        assert_eq!(
            error
                .downcast_ref::<crate::gmail::GmailApiError>()
                .unwrap()
                .status()
                .as_u16()
                .to_string(),
            &status[..3]
        );
    }
}

#[test]
fn priority_a_label_generic_response_accepts_exact_cap() {
    let template = r#"{"labels":[],"padding":""}"#;
    let body = format!(
        r#"{{"labels":[],"padding":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024 - template.len())
    );
    for chunked in [false, true] {
        let (base_url, server) = priority_a_label_framed_response(body.clone(), chunked);
        let result = GmailApi::with_base_url(&base_url)
            .unwrap()
            .list_labels("test-token");
        server.join().unwrap();
        assert!(result.unwrap().labels.is_empty());
    }
}

#[test]
fn priority_a_label_create_oversized_success_remains_uncertain() {
    let body = format!(
        r#"{{"id":"Label_7","name":"Atlas","type":"user","padding":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    for chunked in [false, true] {
        let (base_url, server) = priority_a_label_framed_response(body.clone(), chunked);
        let error = GmailApi::with_base_url(&base_url)
            .unwrap()
            .create_label(
                "test-token",
                crate::gmail::CreateLabelRequest {
                    name: "Atlas".into(),
                },
            )
            .unwrap_err();
        server.join().unwrap();
        assert!(GmailApi::is_label_write_uncertain(&error));
        assert!(!crate::gmail::is_unauthorized(&error));
        assert!(!format!("{error:#}").contains("padding"));
    }
}

#[test]
fn label_write_internal_diagnostics_are_sanitized_and_stage_specific() {
    let (base_url, server) = mock_gmail_response("private-provider-body".into(), "200 OK");
    let error = GmailApi::with_base_url(&base_url)
        .unwrap()
        .create_label(
            "synthetic-private-token",
            crate::gmail::CreateLabelRequest {
                name: "Atlas".into(),
            },
        )
        .unwrap_err();
    server.join().unwrap();
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains("stage=create_response"), "{diagnostic}");
    assert!(diagnostic.contains("status=200"), "{diagnostic}");
    assert!(diagnostic.contains("category=response"), "{diagnostic}");
    assert!(!diagnostic.contains("private-provider-body"));
    assert!(!diagnostic.contains("synthetic-private-token"));
    assert!(!diagnostic.contains(&base_url));
    assert!(!crate::gmail::is_unauthorized(&error));
}

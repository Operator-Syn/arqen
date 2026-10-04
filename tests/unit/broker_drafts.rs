// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

fn state() -> BrokerState {
    BrokerState {
        database_path: PathBuf::from("unused"),
        credentials_path: PathBuf::from("unused"),
        access_tokens: Default::default(),
        pending_actions: Default::default(),
    }
}

// Synthetic HTTP provider only; no Google calls or credentials.
fn provider(revision: &str) -> (GmailApi, mpsc::Sender<()>, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api =
        GmailApi::with_base_url(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    listener.set_nonblocking(true).unwrap();
    let revision = revision.to_owned();
    let (stop, stopped) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while stopped.try_recv().is_err() && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" {
                    break;
                }
                if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let response = if first.starts_with("GET ") {
                format!(
                    r#"{{"id":"draft-1","message":{{"id":"{revision}","threadId":"thread-1"}}}}"#
                )
            } else if first.starts_with("DELETE ") {
                String::new()
            } else {
                r#"{"id":"sent-1","threadId":"thread-1"}"#.to_owned()
            };
            requests.push(first.trim().to_owned());
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
        requests
    });
    (api, stop, worker)
}

// Bounded local fixture; never contacts Gmail or returns captured credentials.
fn draft_listing_error(responses: Vec<(&str, String)>) -> anyhow::Error {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let api =
        GmailApi::with_base_url(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let responses: Vec<_> = responses
        .into_iter()
        .map(|(s, b)| (s.to_owned(), b))
        .collect();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        for (status, body) in responses {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "fixture request deadline exceeded"
                        );
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                assert!(!line.is_empty());
            }
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            // Oversized advertised lengths are rejected before reading a body.
            if body.len() <= 2 * 1024 * 1024 {
                stream.write_all(body.as_bytes()).unwrap();
            }
        }
    });
    let error = api
        .list_drafts("private-token", "selected@example.com", Default::default())
        .unwrap_err();
    worker.join().unwrap();
    error
}

#[test]
fn disappearing_draft_detail_returns_all_or_nothing_retry_guidance() {
    let error = draft_listing_error(vec![
        ("200 OK", r#"{"drafts":[{"id":"draft-1"},{"id":"draft-2"}]}"#.into()),
        ("200 OK", r#"{"id":"draft-1","message":{"id":"message-1","threadId":"thread-1","labelIds":["DRAFT"]}}"#.into()),
        ("404 Not Found", "private-provider-body".into()),
    ]);
    let response = map_draft_error(&error, "list drafts");
    assert!(
        matches!(&response, BrokerResponse::Error { code: BrokerErrorCode::MessageNotFound, message }
        if message.contains("No partial page") && message.contains("retry list_drafts"))
    );
    assert!(
        !serde_json::to_string(&response)
            .unwrap()
            .contains("private-provider-body")
    );
}

#[test]
fn draft_listing_validation_preserves_actual_success_status() {
    let error = draft_listing_error(vec![(
        "201 Created",
        r#"{"drafts":[{"id":"private/id"}]}"#.into(),
    )]);
    let diagnostic = error
        .downcast_ref::<crate::gmail::DraftListError>()
        .unwrap();
    assert_eq!(diagnostic.stage, crate::gmail::DraftListStage::List);
    assert_eq!(
        diagnostic.category,
        crate::gmail::DraftListCategory::Validation
    );
    assert_eq!(diagnostic.status, Some(reqwest::StatusCode::CREATED));
}

fn assert_listing_diagnostic(
    error: &anyhow::Error,
    stage: crate::gmail::DraftListStage,
    category: crate::gmail::DraftListCategory,
    status: Option<reqwest::StatusCode>,
) {
    let diagnostic = error
        .downcast_ref::<crate::gmail::DraftListError>()
        .unwrap();
    assert_eq!(diagnostic.stage, stage);
    assert_eq!(diagnostic.category, category);
    assert_eq!(diagnostic.status, status);
    for rendered in [
        format!("{error}"),
        format!("{error:#}"),
        format!("{error:?}"),
        serde_json::to_string(&map_draft_error(error, "list drafts")).unwrap(),
    ] {
        for private in [
            "private",
            "selected@example.com",
            "127.0.0.1",
            "pageToken",
            "draft-1",
        ] {
            assert!(!rendered.contains(private), "leaked {private}: {rendered}");
        }
    }
    assert!(
        error
            .chain()
            .all(|source| source.downcast_ref::<reqwest::Error>().is_none()
                && source.downcast_ref::<serde_json::Error>().is_none())
    );
}

#[test]
fn draft_listing_classifies_provider_status_and_preserves_public_codes_and_refresh() {
    use crate::gmail::{DraftListCategory, DraftListStage};
    for (status, code, unauthorized) in [
        (
            reqwest::StatusCode::BAD_REQUEST,
            BrokerErrorCode::InvalidRequest,
            false,
        ),
        (
            reqwest::StatusCode::UNAUTHORIZED,
            BrokerErrorCode::ReauthenticationRequired,
            true,
        ),
        (
            reqwest::StatusCode::NOT_FOUND,
            BrokerErrorCode::MessageNotFound,
            false,
        ),
        (
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            BrokerErrorCode::GmailRateLimited,
            false,
        ),
        (
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            BrokerErrorCode::GmailUnavailable,
            false,
        ),
    ] {
        for stage in [DraftListStage::List, DraftListStage::Detail] {
            let mut responses = Vec::new();
            if stage == DraftListStage::Detail {
                responses.push(("200 OK", r#"{"drafts":[{"id":"draft-1"}]}"#.into()));
            }
            let status_line = format!("{} Mock", status.as_u16());
            responses.push((&status_line, "private-provider-body".into()));
            let error = draft_listing_error(responses);
            assert_listing_diagnostic(
                &error,
                stage,
                DraftListCategory::ProviderStatus,
                Some(status),
            );
            assert_eq!(crate::gmail::is_unauthorized(&error), unauthorized);
            assert!(
                matches!(map_draft_error(&error, "list drafts"), BrokerResponse::Error { code: actual, .. } if actual == code)
            );
        }
    }
}

#[test]
fn draft_listing_classifies_decoding_validation_and_response_limits_without_retaining_input() {
    use crate::gmail::{DraftListCategory, DraftListStage};
    for (body, category) in [
        ("private-invalid-json".into(), DraftListCategory::Decoding),
        (
            r#"{"drafts":"private-wrong-type"}"#.into(),
            DraftListCategory::Decoding,
        ),
        (
            r#"{"drafts":[{"id":"private/id"}]}"#.into(),
            DraftListCategory::Validation,
        ),
        (
            "private".repeat(400_000),
            DraftListCategory::ResponseTooLarge,
        ),
    ] {
        let error = draft_listing_error(vec![("200 OK", body)]);
        assert_listing_diagnostic(
            &error,
            DraftListStage::List,
            category,
            Some(reqwest::StatusCode::OK),
        );
        assert!(!crate::gmail::is_unauthorized(&error));
    }
    for (body, category) in [
        ("private-invalid-json".into(), DraftListCategory::Decoding),
        (
            r#"{"id":"private-other-id","message":{"id":"msg-1","threadId":"thread-1"}}"#.into(),
            DraftListCategory::Validation,
        ),
        (
            r#"{"id":"draft-1","message":{"id":"","threadId":"thread-1"}}"#.into(),
            DraftListCategory::Validation,
        ),
        (
            "private".repeat(400_000),
            DraftListCategory::ResponseTooLarge,
        ),
    ] {
        let error = draft_listing_error(vec![
            ("200 OK", r#"{"drafts":[{"id":"draft-1"}]}"#.into()),
            ("200 OK", body),
        ]);
        assert_listing_diagnostic(
            &error,
            DraftListStage::Detail,
            category,
            Some(reqwest::StatusCode::OK),
        );
    }
}

#[test]
fn draft_listing_transport_failure_discards_url_and_token() {
    // Bound a port for deterministic refusal, without a race to reuse it.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    // An open listener that never accepts times out without depending on DNS.
    let api = GmailApi::with_client(&format!("http://{address}/private-url/"), client).unwrap();
    let error = api
        .list_drafts(
            "private-token",
            "selected@example.com",
            crate::gmail::ListDraftsRequest {
                page_token: Some("private-page-token".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert_listing_diagnostic(
        &error,
        crate::gmail::DraftListStage::List,
        crate::gmail::DraftListCategory::Transport,
        None,
    );
    drop(listener);
}

#[test]
fn draft_listing_incomplete_body_is_transport_not_decoding_at_either_stage() {
    use crate::gmail::{DraftListCategory, DraftListStage};
    for stage in [DraftListStage::List, DraftListStage::Detail] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = GmailApi::with_base_url(&format!("http://{}/", listener.local_addr().unwrap()))
            .unwrap();
        let worker = thread::spawn(move || {
            let count = if stage == DraftListStage::Detail {
                2
            } else {
                1
            };
            for index in 0..count {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    assert!(!line.is_empty());
                    if line == "\r\n" {
                        break;
                    }
                }
                if index + 1 == count {
                    // EOF before the promised body completes, not malformed JSON.
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\nprivate").unwrap();
                } else {
                    let body = r#"{"drafts":[{"id":"draft-1"}]}"#;
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
            }
        });
        let error = api
            .list_drafts("private-token", "selected@example.com", Default::default())
            .unwrap_err();
        worker.join().unwrap();
        assert_listing_diagnostic(
            &error,
            stage,
            DraftListCategory::Transport,
            Some(reqwest::StatusCode::OK),
        );
    }
}

#[test]
fn missing_reply_source_guidance_uses_message_id_from_list_emails() {
    let directory =
        std::env::temp_dir().join(format!("arqen-reply-guidance-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut state = state();
    state.database_path = directory.join("accounts.sqlite3");
    let store = AccountStore::open(&state.database_path).unwrap();
    let account = Account {
        id: "account".into(),
        subject: "subject".into(),
        email: "test@example.com".into(),
        display_name: None,
        token_key: Some("keyring:arqen:subject".into()),
        granted_scopes: Some(vec![
            crate::GMAIL_READONLY_SCOPE.into(),
            crate::GMAIL_MODIFY_SCOPE.into(),
        ]),
        connection_state: ConnectionState::Connected,
    };
    store.upsert_google_account(&account).unwrap();
    store.set_mcp_target_subject(Some("subject")).unwrap();
    state.access_tokens.lock().unwrap().insert(
        "subject".into(),
        CachedAccessToken {
            value: "synthetic-token".into(),
            expires_at: Instant::now() + Duration::from_secs(60),
        },
    );
    let response = create_draft_operation(Ok(()), &state, "create reply draft", |_, _, _| {
        Err(GmailApiError::for_test(reqwest::StatusCode::NOT_FOUND, "private source detail").into())
    });
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        matches!(&response, BrokerResponse::Error { code: BrokerErrorCode::MessageNotFound, message }
        if message.contains("source message") && message.contains("message_id from list_emails"))
    );
    assert!(
        !serde_json::to_string(&response)
            .unwrap()
            .contains("private source detail")
    );
}

#[test]
fn persisted_target_change_and_expired_marks_fail_before_credentials() {
    let directory =
        std::env::temp_dir().join(format!("arqen-draft-target-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut state = state();
    state.database_path = directory.join("accounts.sqlite3");
    let store = AccountStore::open(&state.database_path).unwrap();
    for subject in ["subject", "other-subject"] {
        store
            .upsert_google_account(&Account {
                id: subject.into(),
                subject: subject.into(),
                email: format!("{subject}@example.com"),
                display_name: None,
                token_key: Some(format!("keyring:arqen:{subject}")),
                granted_scopes: Some(vec![
                    crate::GMAIL_READONLY_SCOPE.into(),
                    crate::GMAIL_MODIFY_SCOPE.into(),
                ]),
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
    }
    for kind in [PendingActionKind::SendDraft, PendingActionKind::DeleteDraft] {
        let marker =
            register_action_mark(&state, "subject", "revision-1", Some("draft-1"), kind).unwrap();
        let execute = || {
            execute_marked_draft(
                crate::gmail::ActionMarkerRequest {
                    marker_id: marker.clone(),
                },
                kind,
                &state,
            )
        };
        store.set_mcp_target_subject(Some("other-subject")).unwrap();
        assert!(matches!(
            execute(),
            BrokerResponse::Error {
                code: BrokerErrorCode::ActionMarkRequired,
                ..
            }
        ));
        store.set_mcp_target_subject(Some("subject")).unwrap();
        state
            .pending_actions
            .lock()
            .unwrap()
            .get_mut(&("subject".into(), "revision-1".into()))
            .unwrap()
            .expires_at = Instant::now() - Duration::from_secs(1);
        assert!(matches!(
            execute(),
            BrokerResponse::Error {
                code: BrokerErrorCode::ActionMarkRequired,
                ..
            }
        ));
    }
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn changed_revision_errors_require_a_new_action_mark() {
    let error = anyhow::Error::new(DraftRevisionChanged);
    for response in [
        map_send_draft_error(&error),
        map_draft_error(&error, "delete draft"),
    ] {
        assert!(matches!(
            response,
            BrokerResponse::Error {
                code: BrokerErrorCode::ActionMarkRequired,
                ..
            }
        ));
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("changed"));
        assert!(encoded.contains("mark"));
    }
}

#[test]
fn unchanged_draft_revision_is_checked_immediately_before_mutation() {
    for kind in [PendingActionKind::SendDraft, PendingActionKind::DeleteDraft] {
        let state = state();
        let marker =
            register_action_mark(&state, "subject", "revision-1", Some("draft-1"), kind).unwrap();
        let mark = consume_action_mark(&state, &marker, "subject", kind).unwrap();
        let (api, stop, worker) = provider("revision-1");
        let result = execute_draft_action(&api, "synthetic-token", "draft-1", &mark, kind).unwrap();
        stop.send(()).unwrap();
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /users/me/drafts/draft-1?"));
        if kind == PendingActionKind::SendDraft {
            assert_eq!(requests[1], "POST /users/me/drafts/send HTTP/1.1");
            assert_eq!(result.unwrap().message_id, "sent-1");
        } else {
            assert_eq!(requests[1], "DELETE /users/me/drafts/draft-1 HTTP/1.1");
            assert!(result.is_none());
        }
        finish_action_mark(&state, &mark);
        assert!(consume_action_mark(&state, &marker, "subject", kind).is_err());
    }
}

#[test]
fn draft_mark_is_bound_to_account_and_consumed_once_under_concurrency() {
    for kind in [PendingActionKind::SendDraft, PendingActionKind::DeleteDraft] {
        let state = state();
        let marker =
            register_action_mark(&state, "subject", "revision-1", Some("draft-1"), kind).unwrap();
        assert!(consume_action_mark(&state, &marker, "other-subject", kind).is_err());
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let state = state.clone();
                let marker = marker.clone();
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    consume_action_mark(&state, &marker, "subject", kind)
                })
            })
            .collect();
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let mark = results.into_iter().find_map(Result::ok).unwrap();
        let opposite = if kind == PendingActionKind::SendDraft {
            PendingActionKind::DeleteDraft
        } else {
            PendingActionKind::SendDraft
        };
        assert!(matches!(
            register_action_mark(&state, "subject", "revision-2", Some("draft-1"), opposite),
            Err(ActionMarkFailure::InProgress)
        ));
        finish_action_mark(&state, &mark);
        assert!(consume_action_mark(&state, &marker, "subject", kind).is_err());
    }
}

#[test]
fn edited_draft_revision_is_rejected_before_send_or_delete() {
    for kind in [PendingActionKind::SendDraft, PendingActionKind::DeleteDraft] {
        let state = state();
        let marker =
            register_action_mark(&state, "subject", "revision-1", Some("draft-1"), kind).unwrap();
        let mark = consume_action_mark(&state, &marker, "subject", kind).unwrap();
        let (api, stop, worker) = provider("revision-2");
        let result = execute_draft_action(&api, "synthetic-token", "draft-1", &mark, kind);
        stop.send(()).unwrap();
        let requests = worker.join().unwrap();
        assert!(
            result.is_err(),
            "an edited draft must require a new mark: {requests:?}"
        );
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /users/me/drafts/draft-1?"));
        finish_action_mark(&state, &mark);
        assert!(consume_action_mark(&state, &marker, "subject", kind).is_err());
    }
}

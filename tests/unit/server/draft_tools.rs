// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn draft_tool_calls_forward_distinct_broker_operations_and_typed_results() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        thread,
    };

    let socket_path =
        std::env::temp_dir().join(format!("ad-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker = thread::spawn(move || {
        let responses = [
            BrokerResponse::Drafts {
                result: arqen::gmail::DraftListResponse {
                    target_email: "target@example.com".into(),
                    drafts: vec![],
                    next_page_token: None,
                    result_size_estimate: Some(0),
                },
            },
            BrokerResponse::DraftCreated {
                result: arqen::gmail::DraftCreateResult {
                    draft_id: "reply-draft".into(),
                    message_id: "reply-message".into(),
                    thread_id: "thread-1".into(),
                },
            },
            BrokerResponse::DraftCreated {
                result: arqen::gmail::DraftCreateResult {
                    draft_id: "new-draft".into(),
                    message_id: "new-message".into(),
                    thread_id: "new-thread".into(),
                },
            },
            BrokerResponse::DraftDeletionMarked {
                result: arqen::gmail::DraftActionMark {
                    marker_id: "a".repeat(32),
                    draft_id: "draft-1".into(),
                    expires_in_seconds: 600,
                },
            },
            BrokerResponse::DraftDeleted {
                result: arqen::gmail::DraftDeleteResult {
                    draft_id: "draft-1".into(),
                    deleted: true,
                },
            },
            BrokerResponse::DraftSendingMarked {
                result: arqen::gmail::DraftActionMark {
                    marker_id: "b".repeat(32),
                    draft_id: "draft-1".into(),
                    expires_in_seconds: 600,
                },
            },
            BrokerResponse::DraftSent {
                result: arqen::gmail::DraftSendResult {
                    draft_id: "draft-1".into(),
                    message_id: "sent-1".into(),
                    thread_id: "thread-1".into(),
                },
            },
        ];
        for response in responses {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            assert!(matches!(
                request.validate().unwrap(),
                BrokerRequest::ListDrafts { .. }
                    | BrokerRequest::CreateReplyDraft { .. }
                    | BrokerRequest::CreateDraft { .. }
                    | BrokerRequest::MarkDraftForDeletion { .. }
                    | BrokerRequest::DeleteMarkedDraft { .. }
                    | BrokerRequest::MarkDraftForSending { .. }
                    | BrokerRequest::SendMarkedDraft { .. }
            ));
            let mut stream = reader.into_inner();
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });

    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = tcp_listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let mut options = test_options(address);
    options.broker_socket = socket_path.clone();
    let router = build_router(options, cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(tcp_listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });

    let calls = [
        (
            "list_drafts".to_owned(),
            r#"{}"#.to_owned(),
            "target_email".to_owned(),
        ),
        (
            "create_reply_draft".to_owned(),
            r#"{"message_id":"source-1","body":"Reply"}"#.to_owned(),
            "reply-draft".to_owned(),
        ),
        (
            "create_draft".to_owned(),
            r#"{"to":"person@example.com","subject":"Hello","body":"Body"}"#.to_owned(),
            "new-draft".to_owned(),
        ),
        (
            "mark_draft_for_deletion".to_owned(),
            r#"{"draft_id":"draft-1"}"#.to_owned(),
            "draft-1".to_owned(),
        ),
        (
            "delete_marked_draft".to_owned(),
            format!(r#"{{"marker_id":"{}"}}"#, "a".repeat(32)),
            "draft-1".to_owned(),
        ),
        (
            "mark_draft_for_sending".to_owned(),
            r#"{"draft_id":"draft-1"}"#.to_owned(),
            "draft-1".to_owned(),
        ),
        (
            "send_marked_draft".to_owned(),
            format!(r#"{{"marker_id":"{}"}}"#, "b".repeat(32)),
            "sent-1".to_owned(),
        ),
    ];
    for (id, (name, arguments, expected)) in calls.iter().enumerate() {
        let response = reqwest::Client::new().post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret").header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(format!(r#"{{"jsonrpc":"2.0","id":{},"method":"tools/call","params":{{"name":"{}","arguments":{}}}}}"#, id + 1, name, arguments))
            .send().await.unwrap();
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["result"]["isError"], false, "{name}: {body}");
        assert!(
            body["result"]["structuredContent"]
                .to_string()
                .contains(expected),
            "{name}: {body}"
        );
    }
    broker.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

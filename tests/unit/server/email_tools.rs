// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn tool_call_forwards_only_the_bounded_list_request_to_the_broker() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        thread,
    };

    let socket_path =
        std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker_thread = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let request: BrokerRequest = serde_json::from_str(&line).unwrap();
        let request = match request.validate().unwrap() {
            BrokerRequest::ListEmails { request } => request,
            other => panic!("unexpected broker request: {other:?}"),
        };
        assert_eq!(request.query.as_deref(), Some("from:sender@example.com"));
        assert_eq!(request.max_results, 3);
        assert_eq!(request.page_token.as_deref(), Some("next-page-token"));
        let response = BrokerResponse::Ok {
            result: EmailListResponse {
                target_email: "target@example.com".into(),
                messages: Vec::new(),
                next_page_token: Some("following-page-token".into()),
                result_size_estimate: Some(0),
            },
        };
        let mut stream = reader.into_inner();
        serde_json::to_writer(&mut stream, &response).unwrap();
        stream.write_all(b"\n").unwrap();
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
    let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_emails","arguments":{"query":" from:sender@example.com ","max_results":3,"page_token":"next-page-token"}}}"#)
            .send()
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["target_email"],
        "target@example.com"
    );
    assert_eq!(
        body["result"]["structuredContent"]["next_page_token"],
        "following-page-token"
    );
    broker_thread.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

#[cfg(unix)]
#[tokio::test]
async fn read_email_forwards_only_message_id_and_returns_the_broker_result() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        thread,
    };

    let socket_path =
        std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker_thread = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let request: BrokerRequest = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            request.validate().unwrap(),
            BrokerRequest::ReadEmail { request }
                if request.message_id == "message-123"
        ));
        let response = BrokerResponse::ReadEmail {
            result: EmailReadResponse {
                message_id: "message-123".into(),
                thread_id: "thread-456".into(),
                from: Some("sender@example.com".into()),
                recipients: EmailRecipients {
                    to: vec!["recipient@example.com".into()],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                },
                date: Some("Mon, 1 Jan 2024 00:00:00 +0000".into()),
                subject: Some("Subject".into()),
                labels: vec!["INBOX".into()],
                body_text: Some("Full message text".into()),
                body_status: EmailBodyStatus::Complete,
            },
        };
        let mut stream = reader.into_inner();
        serde_json::to_writer(&mut stream, &response).unwrap();
        stream.write_all(b"\n").unwrap();
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
    let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"read_email","arguments":{"message_id":"message-123"}}}"#)
            .send()
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["message_id"],
        "message-123"
    );
    assert_eq!(
        body["result"]["structuredContent"]["body_text"],
        "Full message text"
    );
    assert_eq!(
        body["result"]["structuredContent"]["body_status"],
        "complete"
    );
    broker_thread.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

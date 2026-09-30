// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn mark_email_tools_forward_distinct_operations_and_return_only_read_state() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        thread,
    };

    let socket_path =
        std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker_thread = thread::spawn(move || {
        let operations = [("mark_email_read", true), ("mark_email_unread", false)];
        for (operation, is_read) in operations {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            match request.validate().unwrap() {
                BrokerRequest::MarkEmailRead { request }
                    if operation == "mark_email_read" && request.message_id == "message-123" => {}
                BrokerRequest::MarkEmailUnread { request }
                    if operation == "mark_email_unread" && request.message_id == "message-123" => {}
                _ => panic!("unexpected broker operation for {operation}"),
            }
            let response = BrokerResponse::MessageReadState {
                result: arqen::gmail::EmailReadState {
                    message_id: "message-123".into(),
                    is_read,
                },
            };
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

    for (id, name, expected_is_read) in [
        (70, "mark_email_read", true),
        (71, "mark_email_unread", false),
    ] {
        let response = reqwest::Client::new()
                .post(format!("http://{address}/mcp"))
                .bearer_auth("test-secret")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{"message_id":"message-123"}}}}}}"#
                ))
                .send()
                .await
                .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(
            body["result"]["structuredContent"],
            serde_json::json!({"message_id":"message-123","is_read":expected_is_read})
        );
    }
    broker_thread.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

#[tokio::test]
async fn mark_email_tools_reject_account_thread_and_unknown_fields() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });

    for (id, name, extra_field) in [
        (80, "mark_email_read", "account_id"),
        (81, "mark_email_read", "thread_id"),
        (82, "mark_email_unread", "account_id"),
        (83, "mark_email_unread", "thread_id"),
        (84, "mark_email_read", "unexpected"),
        (85, "mark_email_unread", "unexpected"),
    ] {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{"message_id":"message-123","{extra_field}":"other"}}}}}}"#
        );
        let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body)
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["result"]["isError"], true);
        assert!(
            body["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("unknown field")
        );
    }
    cancellation.cancel();
}

#[tokio::test]
async fn mark_email_tools_return_stable_malformed_message_id_errors() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });

    for (id, name) in [(84, "mark_email_read"), (85, "mark_email_unread")] {
        let response = reqwest::Client::new()
                .post(format!("http://{address}/mcp"))
                .bearer_auth("test-secret")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{{"message_id":"bad/id"}}}}}}"#
                ))
                .send()
                .await
                .unwrap();
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["result"]["isError"], true);
        assert_eq!(
            body["result"]["content"][0]["text"],
            "invalid_message_id: message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
        );
    }
    cancellation.cancel();
}

#[tokio::test]
async fn malformed_read_email_ids_return_a_stable_validation_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });
    let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"read_email","arguments":{"message_id":"bad/id"}}}"#)
            .send()
            .await
            .unwrap();
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(
        body["result"]["content"][0]["text"],
        "invalid_message_id: message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
    );
    cancellation.cancel();
}

#[tokio::test]
async fn read_email_rejects_account_selection_arguments() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });
    let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"read_email","arguments":{"message_id":"message-123","account_id":"other-account"}}}"#)
            .send()
            .await
            .unwrap();
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["result"]["isError"], true);
    assert!(
        body["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown field `account_id`")
    );
    cancellation.cancel();
}

#[cfg(unix)]
#[tokio::test]
async fn invalid_page_sizes_return_the_stable_constraint_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });

    for (id, max_results) in [(10, 0), (11, 51)] {
        let response = reqwest::Client::new()
                .post(format!("http://{address}/mcp"))
                .bearer_auth("test-secret")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"list_emails","arguments":{{"max_results":{max_results}}}}}}}"#
                ))
                .send()
                .await
                .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["result"]["isError"], true);
        assert_eq!(
            body["result"]["content"][0]["text"],
            "invalid_request: max_results must be between 1 and 50"
        );
    }

    cancellation.cancel();
}

// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn list_labels_takes_no_arguments_and_returns_broker_labels() {
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
            BrokerRequest::ListLabels
        ));
        let response = BrokerResponse::Labels {
            result: LabelListResponse {
                labels: vec![EmailLabel {
                    id: "Label_7".into(),
                    name: "Project Atlas".into(),
                    label_type: EmailLabelType::User,
                }],
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
            .body(r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"list_labels","arguments":{}}}"#)
            .send()
            .await
            .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["labels"][0],
        serde_json::json!({"id":"Label_7", "name":"Project Atlas", "type":"user"})
    );
    broker_thread.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

#[cfg(unix)]
#[tokio::test]
async fn list_labels_id_is_passed_unchanged_to_separate_delete_label_call() {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        thread,
    };

    let socket_path =
        std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker_thread = thread::spawn(move || {
        for expected in 0..3 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            let response = if expected == 0 {
                assert!(matches!(
                    request.validate().unwrap(),
                    BrokerRequest::ListLabels
                ));
                BrokerResponse::Labels {
                    result: LabelListResponse {
                        labels: vec![EmailLabel {
                            id: "Label_7".into(),
                            name: "Project Atlas".into(),
                            label_type: EmailLabelType::User,
                        }],
                    },
                }
            } else if expected == 1 {
                match request.validate().unwrap() {
                    BrokerRequest::DeleteLabel { request } => {
                        assert_eq!(request.label_id, "Label_7");
                    }
                    other => panic!("unexpected broker request: {other:?}"),
                }
                BrokerResponse::LabelDeleted {
                    result: arqen::gmail::LabelDeleteResult {
                        label_id: "Label_7".into(),
                        deleted: true,
                    },
                }
            } else {
                match request.validate().unwrap() {
                    BrokerRequest::CreateLabel { request } => {
                        assert_eq!(request.name, "Follow up");
                    }
                    other => panic!("unexpected broker request: {other:?}"),
                }
                BrokerResponse::LabelCreated {
                    result: EmailLabel {
                        id: "Label_8".into(),
                        name: "Follow up".into(),
                        label_type: EmailLabelType::User,
                    },
                }
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
    let client = reqwest::Client::new();
    let base = format!("http://{address}/mcp");
    let list_response = client
            .post(&base)
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"list_labels","arguments":{}}}"#)
            .send()
            .await
            .unwrap();
    let listed: serde_json::Value = list_response.json().await.unwrap();
    let id = listed["result"]["structuredContent"]["labels"][0]["id"]
        .as_str()
        .unwrap();
    assert_eq!(id, "Label_7");
    let delete_args = serde_json::json!({"label_id": id});
    let delete_request = serde_json::json!({
        "jsonrpc":"2.0",
        "id":12,
        "method":"tools/call",
        "params":{"name":"delete_label","arguments":delete_args}
    });
    let delete_response = client
        .post(&base)
        .bearer_auth("test-secret")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(delete_request.to_string())
        .send()
        .await
        .unwrap();
    let deleted: serde_json::Value = delete_response.json().await.unwrap();
    assert_eq!(
        deleted["result"]["structuredContent"],
        serde_json::json!({"label_id":"Label_7","deleted":true})
    );
    let create_response = client
            .post(&base)
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"create_label","arguments":{"name":"Follow up"}}}"#)
            .send()
            .await
            .unwrap();
    let created: serde_json::Value = create_response.json().await.unwrap();
    assert_eq!(
        created["result"]["structuredContent"],
        serde_json::json!({"id":"Label_8","name":"Follow up","type":"user"})
    );
    broker_thread.join().unwrap();
    cancellation.cancel();
    let _ = std::fs::remove_file(socket_path);
}

#[tokio::test]
async fn label_tools_reject_account_selectors_and_unknown_fields() {
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

    for (id, name, arguments) in [
        (
            90,
            "create_label",
            r#"{"name":"Inbox","account_id":"other"}"#,
        ),
        (
            91,
            "create_label",
            r#"{"name":"Inbox","email":"other@example.com"}"#,
        ),
        (
            92,
            "delete_label",
            r#"{"label_id":"Label_7","account_id":"other"}"#,
        ),
        (
            93,
            "delete_label",
            r#"{"label_id":"Label_7","name":"Inbox"}"#,
        ),
        (
            94,
            "delete_label",
            r#"{"label_id":"Label_7","confirmed":true}"#,
        ),
    ] {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{name}","arguments":{arguments}}}}}"#
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

    let blank_name = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":95,"method":"tools/call","params":{"name":"create_label","arguments":{"name":"   "}}}"#)
            .send()
            .await
            .unwrap();
    let body: serde_json::Value = blank_name.json().await.unwrap();
    assert_eq!(body["result"]["isError"], true);
    assert!(
        body["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("invalid_label_name")
    );
    cancellation.cancel();
}

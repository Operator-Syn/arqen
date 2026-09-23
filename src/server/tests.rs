#[cfg(test)]
mod tests {
    use super::{
        ServerOptions, build_router, request_is_authorized, split_values, validate_secret,
    };
    use arqen::gmail::EmailListResponse;
    #[cfg(unix)]
    use arqen::mcp::{BrokerRequest, BrokerResponse};
    use axum::http::Request;
    use reqwest::StatusCode;
    use std::{net::SocketAddr, path::PathBuf};
    use tokio_util::sync::CancellationToken;

    fn test_options(address: SocketAddr) -> ServerOptions {
        ServerOptions {
            listen_addr: address,
            broker_socket: PathBuf::from("/tmp/arqen-test-missing-broker.sock"),
            allowed_hosts: vec![address.to_string()],
            allowed_origins: vec![format!("http://{address}")],
            bearer_token: "test-secret".into(),
        }
    }

    #[test]
    fn bearer_authorization_requires_exact_scheme_and_value() {
        let request = Request::builder()
            .header("authorization", "Bearer secret")
            .body(())
            .unwrap();
        assert!(request_is_authorized(&request, "secret"));
        let wrong = Request::builder()
            .header("authorization", "Bearer other")
            .body(())
            .unwrap();
        assert!(!request_is_authorized(&wrong, "secret"));
        let basic = Request::builder()
            .header("authorization", "Basic secret")
            .body(())
            .unwrap();
        assert!(!request_is_authorized(&basic, "secret"));
    }

    #[test]
    fn bearer_secret_validation_rejects_empty_and_control_values() {
        assert!(validate_secret("", "token").is_err());
        assert!(validate_secret("secret\n", "token").is_err());
        assert!(validate_secret("secret", "token").is_ok());
    }

    #[test]
    fn split_list_values_require_an_explicit_environment_value() {
        assert_eq!(
            split_values("ARQEN_TEST_LIST", "example.com, https://example.com").unwrap(),
            vec!["example.com", "https://example.com"]
        );
    }

    #[tokio::test]
    async fn http_surface_requires_bearer_auth_and_exposes_tools() {
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
        let client = reqwest::Client::new();
        let base = format!("http://{address}");

        let unauthorized = client.get(format!("{base}/healthz")).send().await.unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthorized
                .headers()
                .get("www-authenticate")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer realm=\"arqen-mcp\"")
        );

        let health = client
            .get(format!("{base}/healthz"))
            .bearer_auth("test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::NO_CONTENT);

        let readiness = client
            .get(format!("{base}/readyz"))
            .bearer_auth("test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(readiness.status(), StatusCode::SERVICE_UNAVAILABLE);
        let readiness_body: serde_json::Value = readiness.json().await.unwrap();
        assert_eq!(readiness_body["code"], "internal");

        let initialize = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(initialize.status(), StatusCode::OK);
        let body: serde_json::Value = initialize.json().await.unwrap();
        assert!(body["result"]["capabilities"]["tools"].is_object());

        let tools = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("origin", format!("http://{address}"))
            .body(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(tools.status(), StatusCode::OK);
        let body: serde_json::Value = tools.json().await.unwrap();
        assert_eq!(body["result"]["tools"][0]["name"], "list_emails");
        let tool = &body["result"]["tools"][0];
        let schema = &tool["inputSchema"];
        let properties = &schema["properties"];
        assert_eq!(properties.as_object().unwrap().len(), 5);
        assert!(tool["description"].as_str().unwrap().contains("single account selected in Arqen"));
        for response_field in [
            "target_email",
            "messages",
            "next_page_token",
            "result_size_estimate",
        ] {
            assert!(tool["description"].as_str().unwrap().contains(response_field));
        }
        assert!(tool["description"].as_str().unwrap().contains("Results may be paginated"));
        assert!(schema_supports_type(&properties["query"], "string"));
        assert_eq!(properties["query"]["default"], "in:inbox");
        assert_eq!(properties["query"]["maxLength"], 1_024);
        assert!(properties["query"]["pattern"].is_string());
        assert!(schema_supports_null(&properties["query"]));
        assert!(schema_supports_type(&properties["label_ids"], "array"));
        assert_eq!(properties["label_ids"]["maxItems"], 20);
        assert_eq!(properties["label_ids"]["items"]["minLength"], 1);
        assert_eq!(properties["label_ids"]["items"]["maxLength"], 256);
        assert!(properties["label_ids"]["items"]["pattern"].is_string());
        assert_eq!(properties["label_ids"]["default"], serde_json::json!([]));
        assert!(schema_supports_type(&properties["max_results"], "integer"));
        assert_eq!(properties["max_results"]["minimum"], 1);
        assert_eq!(properties["max_results"]["maximum"], 50);
        assert_eq!(properties["max_results"]["default"], 20);
        assert_eq!(properties["page_token"]["maxLength"], 4_096);
        assert_eq!(properties["page_token"]["minLength"], 1);
        assert!(properties["page_token"]["pattern"].is_string());
        assert_eq!(properties["page_token"]["default"], serde_json::Value::Null);
        assert!(schema_supports_null(&properties["page_token"]));
        assert!(schema_supports_type(
            &properties["include_spam_trash"],
            "boolean"
        ));
        assert_eq!(properties["include_spam_trash"]["default"], false);
        assert!(schema["required"].as_array().is_none_or(Vec::is_empty));
        assert!(properties["query"]["description"]
            .as_str()
            .unwrap()
            .contains("in:inbox"));
        assert!(properties["page_token"]["description"]
            .as_str()
            .unwrap()
            .contains("next page"));
        assert!(properties["label_ids"]["description"]
            .as_str()
            .unwrap()
            .contains("at most 20 IDs"));
        assert!(properties["include_spam_trash"]["description"]
            .as_str()
            .unwrap()
            .contains("spam and trash"));
        assert!(properties["max_results"]["description"]
            .as_str()
            .unwrap()
            .contains("1–50"));

        let rejected_host = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("host", "unexpected.example")
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected_host.status(), StatusCode::FORBIDDEN);

        let rejected_origin = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("origin", "https://unexpected.example")
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":5,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected_origin.status(), StatusCode::FORBIDDEN);

        cancellation.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tool_call_forwards_only_the_bounded_list_request_to_the_broker() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixListener,
            thread,
        };

        let socket_path = std::env::temp_dir().join(format!(
            "arqen-server-test-{}.sock",
            uuid::Uuid::new_v4().simple()
        ));
        let listener = UnixListener::bind(&socket_path).unwrap();
        let broker_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            let request = request.validate().unwrap();
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

    fn schema_supports_null(schema: &serde_json::Value) -> bool {
        schema_supports_type(schema, "null")
            || schema["anyOf"]
            .as_array()
            .is_some_and(|variants| variants.iter().any(|variant| variant["type"] == "null"))
    }

    fn schema_supports_type(schema: &serde_json::Value, expected: &str) -> bool {
        schema["type"] == expected
            || schema["type"]
                .as_array()
                .is_some_and(|types| types.iter().any(|kind| kind == expected))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn readiness_reports_ready_when_the_broker_accepts_status() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixListener,
            thread,
        };

        let socket_path = std::env::temp_dir().join(format!(
            "arqen-server-readiness-test-{}.sock",
            uuid::Uuid::new_v4().simple()
        ));
        let listener = UnixListener::bind(&socket_path).unwrap();
        let broker_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            assert!(request.validate_readiness().is_ok());
            let mut stream = reader.into_inner();
            serde_json::to_writer(&mut stream, &BrokerResponse::Ready).unwrap();
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
            .get(format!("http://{address}/readyz"))
            .bearer_auth("test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        broker_thread.join().unwrap();
        cancellation.cancel();
        let _ = std::fs::remove_file(socket_path);
    }
}

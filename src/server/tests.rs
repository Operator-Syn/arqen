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
            let response = BrokerResponse::Ok {
                result: EmailListResponse {
                    target_email: "target@example.com".into(),
                    messages: Vec::new(),
                    next_page_token: None,
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
            .body(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_emails","arguments":{"query":" from:sender@example.com ","max_results":3}}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(
            body["result"]["structuredContent"]["target_email"],
            "target@example.com"
        );
        broker_thread.join().unwrap();
        cancellation.cancel();
        let _ = std::fs::remove_file(socket_path);
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

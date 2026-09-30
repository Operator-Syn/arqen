// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn readiness_reports_ready_when_the_broker_accepts_status() {
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

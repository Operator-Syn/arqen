// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
pub(super) async fn scripted_server(
    script: Vec<(BrokerRequest, BrokerResponse)>,
) -> (
    SocketAddr,
    CancellationToken,
    std::thread::JoinHandle<()>,
    PathBuf,
) {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
    };
    let socket_path =
        std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = UnixListener::bind(&socket_path).unwrap();
    let broker = std::thread::spawn(move || {
        for (expected, response) in script {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let actual: BrokerRequest = serde_json::from_str(&line).unwrap();
            assert_eq!(actual, expected, "exact operation and every argument");
            assert_eq!(actual.clone().validate().unwrap(), expected);
            let mut stream = reader.into_inner();
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        }
        listener.set_nonblocking(true).unwrap();
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "no hidden broker calls"
        );
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let mut options = test_options(address);
    options.broker_socket = socket_path.clone();
    let router = build_router(options, cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    (address, cancellation, broker, socket_path)
}

pub(super) async fn mcp_request(
    address: SocketAddr,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    reqwest::Client::new()
        .post(format!("http://{address}/mcp"))
        .bearer_auth("test-secret")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub(super) async fn test_server() -> (SocketAddr, CancellationToken) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let router = build_router(test_options(address), cancellation.clone());
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    (address, cancellation)
}

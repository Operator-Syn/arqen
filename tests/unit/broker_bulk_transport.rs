// SPDX-License-Identifier: MPL-2.0
use super::*;
#[tokio::test]
async fn bulk_transport_rejects_oversize_before_connect() {
    let client = BrokerClient::new("/nonexistent/arqen-bulk-test.sock");
    let draft = crate::gmail::CreateDraftRequest {
        to: "test@example.com".into(),
        subject: "Test".into(),
        body: "🦀".repeat(20_000),
    };
    let error = client
        .create_drafts(crate::gmail::CreateDraftsRequest {
            drafts: vec![draft; 3],
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, BrokerErrorCode::InvalidRequest);
}
#[cfg(unix)]
#[tokio::test]
async fn bulk_transport_forwards_exactly_one_request() {
    use std::os::unix::net::UnixListener;
    let path = std::env::temp_dir().join(format!("bulk-{}.sock", uuid::Uuid::new_v4()));
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::yield_now()
                }
                Err(e) => panic!("{e}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let frame = read_bounded_line(
            &mut BufReader::new(stream.try_clone().unwrap()),
            MAX_REQUEST_BYTES,
        )
        .unwrap();
        let request: BrokerRequest = serde_json::from_slice(&frame).unwrap();
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            serde_json::json!({"operation":"mark_emails_read","message_ids":["m2","m1"]})
        );
        stream.write_all(b"{\"status\":\"messages_read_state\",\"result\":{\"items\":[{\"index\":0,\"status\":\"succeeded\",\"result\":{\"message_id\":\"m2\",\"is_read\":true}}]}}\n").unwrap();
    });
    let result = BrokerClient::new(&path)
        .mark_emails_read(crate::gmail::BulkMessageIdsRequest {
            message_ids: vec!["m2".into(), "m1".into()],
        })
        .await;
    worker.join().unwrap();
    let _ = fs::remove_file(path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().items[0].index, 0);
}

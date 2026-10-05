// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::sync::CancellationToken;

async fn mcp(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let result:serde_json::Value=client.post(format!("http://{address}/mcp")).bearer_auth("test-secret").header("accept","application/json, text/event-stream").json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}})).send().await.unwrap().json().await.unwrap();
    assert_ne!(result["result"]["isError"], true, "{result}");
    let content = result["result"]["structuredContent"].clone();
    assert!(serde_json::to_vec(&content).unwrap().len() <= MAX_BULK_RESULT_BYTES);
    content
}
fn account_items(result: &serde_json::Value, ids: &[String], field: &str) {
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), ids.len());
    for (i, (item, id)) in items.iter().zip(ids).enumerate() {
        assert_eq!(item["index"], i);
        assert_eq!(item["status"], "succeeded", "{item}");
        assert_eq!(item["result"][field], *id);
    }
}
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bulk_scale_500_through_real_http_unix_broker_and_provider() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let a = active.clone();
    let p = peak.clone();
    let labels = Arc::new(Mutex::new(
        std::collections::HashMap::<String, Vec<String>>::new(),
    ));
    let l = labels.clone();
    let provider = fixture::Fixture::new(move |c| {
        let running = a.fetch_add(1, Ordering::SeqCst) + 1;
        p.fetch_max(running, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(1));
        let path = c.path.split('?').next().unwrap();
        let id = path.split('/').next_back().unwrap();
        let response = if path == "/users/me/messages" {
            let url = reqwest::Url::parse(&format!("http://fixture{}", c.path)).unwrap();
            let offset = url
                .query_pairs()
                .find(|(k, _)| k == "pageToken")
                .map(|(_, v)| v.parse::<usize>().unwrap())
                .unwrap_or(0);
            serde_json::json!({"messages":(offset..offset+50).map(|i|serde_json::json!({"id":format!("m{i}"),"threadId":format!("t{i}")})).collect::<Vec<_>>(),"nextPageToken":if offset+50<500{Some((offset+50).to_string())}else{None}})
        } else if path == "/users/me/messages/batchModify" {
            let mut labels = l.lock().unwrap();
            for id in c.body["ids"].as_array().unwrap() {
                let labels = labels
                    .entry(id.as_str().unwrap().into())
                    .or_insert_with(|| vec!["INBOX".into(), "UNREAD".into()]);
                if let Some(add) = c.body["addLabelIds"].as_array() {
                    for v in add {
                        labels.push(v.as_str().unwrap().into());
                    }
                }
                if let Some(remove) = c.body["removeLabelIds"].as_array() {
                    labels.retain(|v| !remove.iter().any(|r| r == v));
                }
            }
            serde_json::json!({})
        } else if path.starts_with("/users/me/labels/") {
            serde_json::json!({"id":id,"type":"user"})
        } else if path.ends_with("/trash") {
            let id = path
                .strip_suffix("/trash")
                .unwrap()
                .split('/')
                .next_back()
                .unwrap();
            l.lock()
                .unwrap()
                .entry(id.into())
                .or_default()
                .push("TRASH".into());
            serde_json::json!({"id":id})
        } else {
            let labels = l
                .lock()
                .unwrap()
                .entry(id.into())
                .or_insert_with(|| vec!["INBOX".into(), "UNREAD".into()])
                .clone();
            serde_json::json!({"id":id,"threadId":format!("t{}",&id[1..]),"labelIds":labels,"payload":{"mimeType":"text/plain","body":{"data":"SGVsbG8","size":5}}})
        };
        a.fetch_sub(1, Ordering::SeqCst);
        Some((200, response))
    });
    let url = provider.url.clone();
    let (s, dir) = tokio::task::spawn_blocking(move || state(&url))
        .await
        .unwrap();
    let socket = std::env::temp_dir().join(format!("a-{}.sock", uuid::Uuid::new_v4().simple()));
    let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let halt = stop.clone();
    let broker_state = s.clone();
    let socket_cleanup = socket.clone();
    let broker = thread::spawn(move || {
        let mut jobs = Vec::new();
        while !halt.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let state = broker_state.clone();
                    jobs.push(thread::spawn(move || handle_connection(stream, &state)));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1))
                }
                Err(e) => panic!("broker fixture accept: {e}"),
            }
        }
        for job in jobs {
            job.join().unwrap();
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let options = crate::server_test_support::ServerOptions {
        listen_addr: address,
        broker_socket: socket,
        allowed_hosts: vec![address.to_string()],
        allowed_origins: vec![format!("http://{address}")],
        bearer_token: "test-secret".into(),
    };
    let router = crate::server_test_support::build_router(options, cancellation.clone());
    let shutdown = cancellation.clone();
    let http = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let started = Instant::now();
    let ids: Vec<_> = (0..500).map(|i| format!("m{i}")).collect();
    let mut page = None;
    let mut listed = Vec::new();
    loop {
        let result = mcp(
            &client,
            address,
            "list_emails",
            serde_json::json!({"query":"in:inbox","max_results":50,"page_token":page}),
        )
        .await;
        listed.extend(
            result["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["id"].as_str().unwrap().to_string()),
        );
        page = result["next_page_token"].as_str().map(str::to_string);
        if page.is_none() {
            break;
        }
    }
    assert_eq!(listed, ids);
    for chunk in ids.chunks(20) {
        let result = mcp(
            &client,
            address,
            "read_emails",
            serde_json::json!({"message_ids":chunk}),
        )
        .await;
        account_items(&result, chunk, "message_id");
        for item in result["items"].as_array().unwrap() {
            assert_eq!(item["result"]["body_text"], "Hello");
        }
    }
    let before_label = provider.snapshot().len();
    for chunk in ids.chunks(100) {
        let result = mcp(
            &client,
            address,
            "apply_label_to_emails",
            serde_json::json!({"message_ids":chunk,"label_id":"Label_fixture"}),
        )
        .await;
        account_items(&result, chunk, "message_id");
    }
    let after_label = provider.snapshot().len();
    assert_eq!(after_label - before_label, 1010); // 500 preflight + 500 readback + 5 label GET + 5 batch POST.
    let snapshot = provider.snapshot();
    let batches: Vec<_> = snapshot
        .iter()
        .filter(|c| c.path == "/users/me/messages/batchModify")
        .collect();
    assert_eq!(batches.len(), 5);
    for (batch, chunk) in batches.iter().zip(ids.chunks(100)) {
        assert_eq!(
            batch.body,
            serde_json::json!({"ids":chunk,"addLabelIds":["Label_fixture"]})
        );
    }
    for chunk in ids.chunks(100) {
        let result = mcp(
            &client,
            address,
            "mark_emails_read",
            serde_json::json!({"message_ids":chunk}),
        )
        .await;
        account_items(&result, chunk, "message_id");
        let marked = mcp(
            &client,
            address,
            "mark_emails_for_deletion",
            serde_json::json!({"message_ids":chunk}),
        )
        .await;
        account_items(&marked, chunk, "message_id");
        let markers: Vec<_> = marked["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["result"]["marker_id"].clone())
            .collect();
        assert!(s.pending_actions.lock().unwrap().len() <= 256);
        let result = mcp(
            &client,
            address,
            "delete_marked_emails",
            serde_json::json!({"marker_ids":markers}),
        )
        .await;
        account_items(&result, chunk, "message_id");
        assert!(s.pending_actions.lock().unwrap().is_empty());
    }
    // Overlapping singular and bulk operations use the same real provider gate.
    let (one, two, three) = tokio::join!(
        mcp(
            &client,
            address,
            "read_emails",
            serde_json::json!({"message_ids":&ids[..20]})
        ),
        mcp(
            &client,
            address,
            "read_emails",
            serde_json::json!({"message_ids":&ids[20..40]})
        ),
        mcp(
            &client,
            address,
            "read_email",
            serde_json::json!({"message_id":"m40"})
        )
    );
    account_items(&one, &ids[..20], "message_id");
    account_items(&two, &ids[20..40], "message_id");
    assert_eq!(three["message_id"], "m40");
    assert!(peak.load(Ordering::SeqCst) <= 8);
    assert!(peak.load(Ordering::SeqCst) > 1);
    assert_eq!(active.load(Ordering::SeqCst), 0);
    let calls = provider.snapshot();
    assert!(!calls.iter().any(|c| c.path.contains("batchDelete")));
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.path.split('?').next().unwrap().ends_with("/trash"))
            .count(),
        500
    );
    for labels in labels.lock().unwrap().values() {
        assert!(labels.contains(&"Label_fixture".into()));
        assert!(labels.contains(&"TRASH".into()));
        assert!(!labels.contains(&"UNREAD".into()));
    }
    eprintln!(
        "Synthetic 500-message HTTP→Unix→broker→provider workload: {} provider calls, 5 native labeling POSTs, label phase {} calls, peak {} requests, observed {:?}",
        calls.len(),
        after_label - before_label,
        peak.load(Ordering::SeqCst),
        started.elapsed()
    );
    cancellation.cancel();
    http.await.unwrap();
    stop.store(true, Ordering::Release);
    broker.join().unwrap();
    tokio::task::spawn_blocking(move || drop(s)).await.unwrap();
    fs::remove_dir_all(dir).unwrap();
    fs::remove_file(socket_cleanup).unwrap();
}

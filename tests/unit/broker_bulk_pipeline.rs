// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bulk_fixture as fixture;
use crate::gmail::*;
#[path = "broker_bulk_failures.rs"]
mod failures;
#[path = "broker_bulk_scale.rs"]
mod scale;
fn state(url: &str) -> (BrokerState, PathBuf) {
    let dir = std::env::temp_dir().join(format!("arqen-bulk-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let database_path = dir.join("accounts.sqlite3");
    let store = AccountStore::open(&database_path).unwrap();
    store
        .upsert_google_account(&Account {
            id: "fixture".into(),
            subject: "subject".into(),
            email: "fixture@example.com".into(),
            display_name: None,
            token_key: Some("fixture-ref".into()),
            granted_scopes: Some(vec![
                crate::GMAIL_READONLY_SCOPE.into(),
                crate::GMAIL_MODIFY_SCOPE.into(),
            ]),
            connection_state: ConnectionState::Connected,
        })
        .unwrap();
    store.set_mcp_target_subject(Some("subject")).unwrap();
    let s = BrokerState {
        api: Arc::new(GmailApi::with_base_url(url).unwrap()),
        refresh_locks: Default::default(),
        database_path,
        credentials_path: dir.join("never-read"),
        access_tokens: Default::default(),
        pending_actions: Default::default(),
    };
    s.access_tokens.lock().unwrap().insert(
        "subject".into(),
        CachedAccessToken {
            value: "synthetic".into(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        },
    );
    (s, dir)
}
#[cfg(unix)]
fn call(state: &BrokerState, value: serde_json::Value) -> serde_json::Value {
    use std::os::unix::net::UnixStream;
    let (mut client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let s = state.clone();
    let worker = thread::spawn(move || handle_connection(server, &s));
    serde_json::to_writer(&mut client, &value).unwrap();
    client.write_all(b"\n").unwrap();
    client.shutdown(std::net::Shutdown::Write).unwrap();
    let bytes = read_bounded_line(&mut BufReader::new(client), MAX_RESPONSE_BYTES).unwrap();
    worker.join().unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
#[cfg(unix)]
#[test]
fn bulk_broker_all_fourteen_operations_are_fully_wired() {
    let labels = Arc::new(Mutex::new(vec!["INBOX".to_string(), "UNREAD".to_string()]));
    let l = labels.clone();
    let provider = fixture::Fixture::new(move |c| {
        let path = c.path.split('?').next().unwrap();
        let id = path.split('/').next_back().unwrap();
        if path == "/users/me/messages/batchModify" {
            let mut labels = l.lock().unwrap();
            if let Some(add) = c.body["addLabelIds"].as_array() {
                for x in add {
                    labels.push(x.as_str().unwrap().into());
                }
            }
            if let Some(remove) = c.body["removeLabelIds"].as_array() {
                labels.retain(|x| !remove.iter().any(|v| v == x));
            }
            return Some((204, serde_json::Value::Null));
        }
        if path == "/users/me/labels" && c.method == "POST" {
            return Some((
                200,
                serde_json::json!({"id":"Label_new","name":c.body["name"],"type":"user"}),
            ));
        }
        if path.starts_with("/users/me/labels/") {
            return Some((
                if c.method == "DELETE" { 204 } else { 200 },
                if c.method == "DELETE" {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({"id":id,"type":"user"})
                },
            ));
        }
        if path == "/users/me/drafts/send" {
            return Some((200, serde_json::json!({"id":"sent","threadId":"thread"})));
        }
        if path.starts_with("/users/me/drafts") {
            return Some((
                if c.method == "DELETE" { 204 } else { 200 },
                if c.method == "DELETE" {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({"id":if id=="drafts"{"created"}else{id},"message":{"id":"revision","threadId":"thread"}})
                },
            ));
        }
        if path.ends_with("/trash") {
            l.lock().unwrap().push("TRASH".into());
            return Some((200, serde_json::json!({"id":"m1"})));
        }
        Some((
            200,
            serde_json::json!({"id":id,"threadId":"thread","labelIds":*l.lock().unwrap(),"payload":{"mimeType":"text/plain","headers":[{"name":"From","value":"sender@example.com"},{"name":"Message-ID","value":"<fixture@example.com>"}],"body":{"data":"SGVsbG8","size":5}}}),
        ))
    });
    let (s, dir) = state(&provider.url);
    for (op, args) in [
        ("read_emails", serde_json::json!({"message_ids":["m1"]})),
        (
            "apply_label_to_emails",
            serde_json::json!({"message_ids":["m1"],"label_id":"Label_1"}),
        ),
        (
            "mark_emails_read",
            serde_json::json!({"message_ids":["m1"]}),
        ),
        (
            "mark_emails_unread",
            serde_json::json!({"message_ids":["m1"]}),
        ),
        ("create_labels", serde_json::json!({"names":["Fixture"]})),
        (
            "delete_labels",
            serde_json::json!({"label_ids":["Label_1"]}),
        ),
        (
            "create_drafts",
            serde_json::json!({"drafts":[{"to":"test@example.com","subject":"Test","body":"Body"}]}),
        ),
        (
            "create_reply_drafts",
            serde_json::json!({"replies":[{"message_id":"m1","body":"Reply"}]}),
        ),
    ] {
        let mut v = args;
        v["operation"] = op.into();
        let result = call(&s, v);
        assert_eq!(
            result["result"]["items"][0]["status"], "succeeded",
            "{op}: {result}"
        );
    }
    for (mark, execute, field, id) in [
        (
            "mark_emails_for_deletion",
            "delete_marked_emails",
            "message_ids",
            "m1",
        ),
        (
            "mark_drafts_for_deletion",
            "delete_marked_drafts",
            "draft_ids",
            "d1",
        ),
        (
            "mark_drafts_for_sending",
            "send_marked_drafts",
            "draft_ids",
            "d2",
        ),
    ] {
        let result = call(&s, serde_json::json!({"operation":mark,field:[id]}));
        assert_eq!(
            result["result"]["items"][0]["status"], "succeeded",
            "{mark}: {result}"
        );
        let marker = result["result"]["items"][0]["result"]["marker_id"].clone();
        let result = call(
            &s,
            serde_json::json!({"operation":execute,"marker_ids":[marker]}),
        );
        assert_eq!(
            result["result"]["items"][0]["status"], "succeeded",
            "{execute}: {result}"
        );
    }
    assert!(s.pending_actions.lock().unwrap().is_empty());
    assert!(
        !provider
            .snapshot()
            .iter()
            .any(|c| c.path.contains("batchDelete"))
    );
    fs::remove_dir_all(dir).unwrap();
}

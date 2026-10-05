// SPDX-License-Identifier: MPL-2.0
use super::*;
#[cfg(unix)]
#[test]
fn bulk_broker_target_and_scope_fail_before_provider_or_credentials() {
    let provider =
        fixture::Fixture::new(|_| panic!("ineligible requests must not contact provider"));
    let (s, dir) = state(&provider.url);
    let store = AccountStore::open(&s.database_path).unwrap();
    store.set_mcp_target_subject(None).unwrap();
    let result = call(
        &s,
        serde_json::json!({"operation":"read_emails","message_ids":["m1"]}),
    );
    assert_eq!(result["code"], "target_not_configured");
    store.set_mcp_target_subject(Some("subject")).unwrap();
    let mut account = store.list_accounts().unwrap().remove(0);
    account.granted_scopes = Some(vec![crate::GMAIL_READONLY_SCOPE.into()]);
    store.upsert_google_account(&account).unwrap();
    s.access_tokens.lock().unwrap().clear();
    let result = call(
        &s,
        serde_json::json!({"operation":"create_labels","names":["Test"]}),
    );
    assert_eq!(result["code"], "insufficient_scope");
    assert!(provider.snapshot().is_empty());
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_create_labels_mixed_conflict_keeps_singular_error_codes() {
    let provider = fixture::Fixture::new(|c| {
        Some(if c.body["name"] == "Existing" {
            (409, serde_json::json!({"private":"secret-provider-body"}))
        } else {
            (
                200,
                serde_json::json!({"id":"Label_new","name":"New","type":"user"}),
            )
        })
    });
    let (s, dir) = state(&provider.url);
    let result = call(
        &s,
        serde_json::json!({"operation":"create_labels","names":["Existing","New"]}),
    );
    assert_eq!(result["result"]["items"][0]["code"], "label_already_exists");
    assert_eq!(result["result"]["items"][1]["status"], "succeeded");
    assert!(!result.to_string().contains("secret-provider-body"));
    assert_eq!(provider.snapshot().len(), 2);
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_delete_labels_preflights_entire_set_and_reconciles_lost_delete() {
    for system in [true, false] {
        let removed = Arc::new(AtomicBool::new(false));
        let r = removed.clone();
        let provider = fixture::Fixture::new(move |c| {
            let id = c
                .path
                .split('?')
                .next()
                .unwrap()
                .split('/')
                .next_back()
                .unwrap();
            if c.method == "DELETE" {
                r.store(true, Ordering::SeqCst);
                return None;
            }
            Some(if r.load(Ordering::SeqCst) {
                (404, serde_json::json!({}))
            } else {
                (
                    200,
                    serde_json::json!({"id":id,"type":if system && id=="INBOX"{"system"}else{"user"}}),
                )
            })
        });
        let (s, dir) = state(&provider.url);
        let result = call(
            &s,
            serde_json::json!({"operation":"delete_labels","label_ids":if system{vec!["Label_1","INBOX"]}else{vec!["Label_1"]}}),
        );
        let deletes = provider
            .snapshot()
            .iter()
            .filter(|c| c.method == "DELETE")
            .count();
        if system {
            assert_eq!(result["code"], "system_label");
            assert_eq!(deletes, 0);
        } else {
            assert_eq!(result["result"]["items"][0]["status"], "succeeded");
            assert_eq!(deletes, 1);
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
#[cfg(unix)]
#[test]
fn bulk_create_drafts_lost_response_is_unknown_and_never_reposted() {
    let provider = fixture::Fixture::new(|_| None);
    let (s, dir) = state(&provider.url);
    let result = call(
        &s,
        serde_json::json!({"operation":"create_drafts","drafts":[{"to":"recipient@example.com","subject":"Fixture","body":"Body"}]}),
    );
    assert_eq!(result["result"]["items"][0]["status"], "unknown");
    assert!(result["result"]["items"][0].get("result").is_none());
    assert_eq!(
        provider
            .snapshot()
            .iter()
            .filter(|c| c.method == "POST")
            .count(),
        1
    );
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_marked_drafts_changed_revision_and_lost_send_are_not_replayed() {
    for changed in [false, true] {
        let marking = Arc::new(AtomicBool::new(true));
        let m = marking.clone();
        let provider = fixture::Fixture::new(move |c| {
            if c.method == "POST" {
                return None;
            }
            Some((
                200,
                serde_json::json!({"id":"d1","message":{"id":if changed && !m.load(Ordering::SeqCst){"edited"}else{"revision"},"threadId":"thread"}}),
            ))
        });
        let (s, dir) = state(&provider.url);
        let result = call(
            &s,
            serde_json::json!({"operation":"mark_drafts_for_sending","draft_ids":["d1"]}),
        );
        let marker = result["result"]["items"][0]["result"]["marker_id"].clone();
        marking.store(false, Ordering::SeqCst);
        let args = serde_json::json!({"operation":"send_marked_drafts","marker_ids":[marker]});
        let result = call(&s, args.clone());
        assert_eq!(
            result["result"]["items"][0]["status"],
            if changed { "failed" } else { "unknown" }
        );
        if changed {
            assert_eq!(result["result"]["items"][0]["code"], "action_mark_required");
        }
        assert!(s.pending_actions.lock().unwrap().is_empty());
        assert_eq!(call(&s, args)["code"], "action_mark_required");
        assert_eq!(
            provider
                .snapshot()
                .iter()
                .filter(|c| c.method == "POST")
                .count(),
            usize::from(!changed)
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
#[cfg(unix)]
#[test]
fn bulk_reads_isolate_missing_and_mismatched_identities() {
    let provider = fixture::Fixture::new(|c| {
        let id = c
            .path
            .split('?')
            .next()
            .unwrap()
            .split('/')
            .next_back()
            .unwrap();
        if id == "missing" {
            return Some((404, serde_json::json!({"private":"hidden"})));
        }
        Some((
            200,
            serde_json::json!({"id":if id=="mismatch"{"other"}else{id},"threadId":"thread","payload":{"mimeType":"text/plain","body":{"data":"SGVsbG8","size":5}}}),
        ))
    });
    let (s, dir) = state(&provider.url);
    let result = call(
        &s,
        serde_json::json!({"operation":"read_emails","message_ids":["m2","missing","mismatch","m1"]}),
    );
    let items = result["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    for (i, item) in items.iter().enumerate() {
        assert_eq!(item["index"], i);
    }
    assert_eq!(items[0]["result"]["body_text"], "Hello");
    assert_eq!(items[1]["code"], "message_not_found");
    assert_eq!(items[2]["status"], "failed");
    assert_eq!(items[3]["result"]["message_id"], "m1");
    assert!(!result.to_string().contains("hidden"));
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_reads_budget_stops_after_dispatched_window_without_losing_indices() {
    use base64::Engine;
    let body = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("\0".repeat(180_000));
    let provider = fixture::Fixture::new(move |c| {
        let id = c
            .path
            .split('?')
            .next()
            .unwrap()
            .split('/')
            .next_back()
            .unwrap();
        Some((
            200,
            serde_json::json!({"id":id,"threadId":"thread","payload":{"mimeType":"text/plain","body":{"data":body,"size":180000}}}),
        ))
    });
    let (s, dir) = state(&provider.url);
    let ids: Vec<_> = (0..20).map(|i| format!("m{i}")).collect();
    let result = call(
        &s,
        serde_json::json!({"operation":"read_emails","message_ids":ids}),
    );
    assert!(serde_json::to_vec(&result).unwrap().len() < MAX_BULK_RESULT_BYTES);
    let items = result["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 20);
    for (i, item) in items.iter().enumerate() {
        assert_eq!(item["index"], i);
        assert_eq!(
            item["status"],
            if i < 4 { "failed" } else { "not_attempted" }
        );
        assert_eq!(item["code"], "response_budget_exceeded");
    }
    assert_eq!(provider.snapshot().len(), 4);
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_broker_admitted_request_retains_captured_target() {
    let (send, recv) = std::sync::mpsc::channel();
    let proceed = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let release = proceed.clone();
    let provider = fixture::Fixture::new(move |c| {
        send.send(()).unwrap();
        let (lock, cv) = &*release;
        let mut go = lock.lock().unwrap();
        while !*go {
            let (guard, timed) = cv.wait_timeout(go, Duration::from_secs(5)).unwrap();
            assert!(!timed.timed_out());
            go = guard;
        }
        let id = c
            .path
            .split('?')
            .next()
            .unwrap()
            .split('/')
            .next_back()
            .unwrap();
        Some((200, serde_json::json!({"id":id,"threadId":"thread"})))
    });
    let (s, dir) = state(&provider.url);
    let worker_state = s.clone();
    let worker = thread::spawn(move || {
        call(
            &worker_state,
            serde_json::json!({"operation":"read_emails","message_ids":["m1","m2"]}),
        )
    });
    recv.recv_timeout(Duration::from_secs(5)).unwrap();
    AccountStore::open(&s.database_path)
        .unwrap()
        .set_mcp_target_subject(None)
        .unwrap();
    *proceed.0.lock().unwrap() = true;
    proceed.1.notify_all();
    let result = worker.join().unwrap();
    assert!(
        result["result"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["status"] == "succeeded")
    );
    assert_eq!(
        call(
            &s,
            serde_json::json!({"operation":"read_emails","message_ids":["m1"]})
        )["code"],
        "target_not_configured"
    );
    fs::remove_dir_all(dir).unwrap();
}
#[cfg(unix)]
#[test]
fn bulk_broker_restart_invalidates_individual_markers() {
    let provider = fixture::Fixture::new(|_| panic!("markers must fail before provider calls"));
    let (s, dir) = state(&provider.url);
    let marked = call(
        &s,
        serde_json::json!({"operation":"mark_emails_for_deletion","message_ids":["m1"]}),
    );
    let marker = marked["result"]["items"][0]["result"]["marker_id"].clone();
    let mut restarted = s.clone();
    restarted.pending_actions = Default::default();
    assert_eq!(
        call(
            &restarted,
            serde_json::json!({"operation":"delete_marked_emails","marker_ids":[marker]})
        )["code"],
        "deletion_mark_required"
    );
    assert!(provider.snapshot().is_empty());
    fs::remove_dir_all(dir).unwrap();
}

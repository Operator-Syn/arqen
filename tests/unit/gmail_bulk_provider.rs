// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bulk_fixture as fixture;
#[path = "gmail_bulk_failures.rs"]
mod failures;
#[test]
fn bulk_label_native_batch_excludes_drafts_and_verifies_exact_state() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let changed = Arc::new(AtomicBool::new(false));
    let c = changed.clone();
    let provider = fixture::Fixture::new(move |call| {
        if call.path.starts_with("/users/me/labels/") {
            return Some((200, serde_json::json!({"id":"Label_1","type":"user"})));
        }
        if call.path.starts_with("/users/me/messages/batchModify") {
            assert_eq!(
                call.body,
                serde_json::json!({"ids":["m2","m1"],"addLabelIds":["Label_1"]})
            );
            c.store(true, Ordering::Relaxed);
            return Some((204, serde_json::Value::Null));
        }
        let id = call
            .path
            .split('/')
            .next_back()
            .unwrap()
            .split('?')
            .next()
            .unwrap();
        let labels = if id == "draft" {
            vec!["DRAFT"]
        } else if c.load(Ordering::Relaxed) {
            vec!["INBOX", "Label_1"]
        } else {
            vec!["INBOX"]
        };
        Some((200, serde_json::json!({"id":id,"labelIds":labels})))
    });
    let api = GmailApi::with_base_url(&provider.url).unwrap();
    let result = api
        .apply_label_to_emails(
            "fixture",
            ApplyLabelToEmailsRequest {
                message_ids: vec!["m2".into(), "draft".into(), "m1".into()],
                label_id: "Label_1".into(),
            },
        )
        .unwrap();
    assert_eq!(result.items.len(), 3);
    assert!(matches!(
        result.items[0].outcome,
        BulkOutcome::Succeeded { .. }
    ));
    assert!(matches!(
        result.items[1].outcome,
        BulkOutcome::Failed { .. }
    ));
    assert!(matches!(
        result.items[2].outcome,
        BulkOutcome::Succeeded { .. }
    ));
    assert_eq!(
        provider
            .snapshot()
            .iter()
            .filter(|c| c.method == "POST")
            .count(),
        1
    );
}

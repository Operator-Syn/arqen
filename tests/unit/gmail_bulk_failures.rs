// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bulk_fixture as fixture;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn request() -> ApplyLabelToEmailsRequest {
    ApplyLabelToEmailsRequest {
        message_ids: vec!["m1".into()],
        label_id: "Label_1".into(),
    }
}
#[test]
fn bulk_label_preflight_401_refreshes_before_any_write() {
    let provider = fixture::Fixture::new(|call| {
        if call.path.starts_with("/users/me/labels/") {
            Some((200, serde_json::json!({"id":"Label_1","type":"user"})))
        } else {
            Some((401, serde_json::json!({"private":"must never escape"})))
        }
    });
    let api = GmailApi::with_base_url(&provider.url).unwrap();
    let result = api.apply_label_to_emails("fixture", request());
    assert!(
        result.as_ref().is_err_and(is_unauthorized),
        "preflight 401 must enter serialized refresh"
    );
    assert!(!provider.snapshot().iter().any(|c| c.method == "POST"));
}
#[test]
fn bulk_label_lost_response_reconciles_without_reposting() {
    for readback in [true, false] {
        let changed = Arc::new(AtomicBool::new(false));
        let c = changed.clone();
        let provider = fixture::Fixture::new(move |call| {
            if call.path.starts_with("/users/me/labels/") {
                return Some((200, serde_json::json!({"id":"Label_1","type":"user"})));
            }
            if call.method == "POST" {
                c.store(true, Ordering::SeqCst);
                return None;
            }
            if c.load(Ordering::SeqCst) && !readback {
                return None;
            }
            Some((
                200,
                serde_json::json!({"id":"m1","labelIds":if c.load(Ordering::SeqCst){vec!["INBOX","Label_1"]}else{vec!["INBOX"]}}),
            ))
        });
        let api = GmailApi::with_base_url(&provider.url).unwrap();
        let result = api.apply_label_to_emails("fixture", request()).unwrap();
        assert_eq!(
            matches!(result.items[0].outcome, BulkOutcome::Succeeded { .. }),
            readback
        );
        assert_eq!(
            matches!(result.items[0].outcome, BulkOutcome::Unknown { .. }),
            !readback
        );
        assert_eq!(
            provider
                .snapshot()
                .iter()
                .filter(|c| c.method == "POST")
                .count(),
            1
        );
    }
}
#[test]
fn bulk_label_provider_statuses_are_sanitized_and_preserve_uncertainty() {
    for status in [400, 401, 403, 404, 429, 500, 200] {
        let provider = fixture::Fixture::new(move |c| {
            if c.path.starts_with("/users/me/labels/") {
                return Some((200, serde_json::json!({"id":"Label_1","type":"user"})));
            }
            if c.method == "POST" {
                return Some((
                    status,
                    serde_json::json!({"private":"hidden-provider-value"}),
                ));
            }
            Some((200, serde_json::json!({"id":"m1","labelIds":["INBOX"]})))
        });
        let api = GmailApi::with_base_url(&provider.url).unwrap();
        let result = api.apply_label_to_emails("fixture", request());
        if status == 401 {
            assert!(result.is_err());
        } else {
            let result = result.unwrap();
            assert!(if status >= 500 || status == 200 {
                matches!(result.items[0].outcome, BulkOutcome::Unknown { .. })
            } else {
                matches!(result.items[0].outcome, BulkOutcome::Failed { .. })
            });
            assert!(
                !serde_json::to_string(&result)
                    .unwrap()
                    .contains("hidden-provider-value")
            );
        }
        assert_eq!(
            provider
                .snapshot()
                .iter()
                .filter(|c| c.method == "POST")
                .count(),
            1
        );
    }
}
#[test]
fn bulk_label_system_and_identity_mismatches_fail_without_post() {
    for (id, kind) in [("Label_1", "system"), ("other", "user")] {
        let provider =
            fixture::Fixture::new(move |_| Some((200, serde_json::json!({"id":id,"type":kind}))));
        let api = GmailApi::with_base_url(&provider.url).unwrap();
        assert!(api.apply_label_to_emails("fixture", request()).is_err());
        assert!(!provider.snapshot().iter().any(|c| c.method == "POST"));
    }
    let provider = fixture::Fixture::new(|c| {
        if c.path.starts_with("/users/me/labels/") {
            Some((200, serde_json::json!({"id":"Label_1","type":"user"})))
        } else {
            Some((200, serde_json::json!({"id":"other","labelIds":["INBOX"]})))
        }
    });
    let api = GmailApi::with_base_url(&provider.url).unwrap();
    assert!(matches!(
        api.apply_label_to_emails("fixture", request())
            .unwrap()
            .items[0]
            .outcome,
        BulkOutcome::Failed { .. }
    ));
    assert!(!provider.snapshot().iter().any(|c| c.method == "POST"));
}
#[test]
fn bulk_read_state_changes_only_unread_in_both_directions() {
    for is_read in [false, true] {
        let changed = Arc::new(AtomicBool::new(false));
        let c = changed.clone();
        let provider = fixture::Fixture::new(move |call| {
            if call.method == "POST" {
                assert_eq!(
                    call.body,
                    if is_read {
                        serde_json::json!({"ids":["m1"],"removeLabelIds":["UNREAD"]})
                    } else {
                        serde_json::json!({"ids":["m1"],"addLabelIds":["UNREAD"]})
                    }
                );
                c.store(true, Ordering::SeqCst);
                return Some((204, serde_json::Value::Null));
            }
            let unread = if c.load(Ordering::SeqCst) {
                !is_read
            } else {
                is_read
            };
            Some((
                200,
                serde_json::json!({"id":"m1","labelIds":if unread{vec!["INBOX","Label_keep","UNREAD"]}else{vec!["INBOX","Label_keep"]}}),
            ))
        });
        let api = GmailApi::with_base_url(&provider.url).unwrap();
        let result = api
            .mark_emails_state(
                "fixture",
                BulkMessageIdsRequest {
                    message_ids: vec!["m1".into()],
                },
                is_read,
            )
            .unwrap();
        assert!(matches!(
            result.items[0].outcome,
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
}

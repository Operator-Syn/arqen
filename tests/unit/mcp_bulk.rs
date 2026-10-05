// SPDX-License-Identifier: MPL-2.0
use super::*;
#[test]
fn bulk_requests_round_trip_with_distinct_operations() {
    for (operation, args) in [
        (
            "read_emails",
            serde_json::json!({"message_ids":["m2","m1"]}),
        ),
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
        (
            "mark_emails_for_deletion",
            serde_json::json!({"message_ids":["m1"]}),
        ),
        (
            "delete_marked_emails",
            serde_json::json!({"marker_ids":["0123456789abcdef0123456789abcdef"]}),
        ),
        ("create_labels", serde_json::json!({"names":["One"]})),
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
            serde_json::json!({"replies":[{"message_id":"m1","body":"Body"}]}),
        ),
        (
            "mark_drafts_for_deletion",
            serde_json::json!({"draft_ids":["d1"]}),
        ),
        (
            "mark_drafts_for_sending",
            serde_json::json!({"draft_ids":["d1"]}),
        ),
        (
            "delete_marked_drafts",
            serde_json::json!({"marker_ids":["0123456789abcdef0123456789abcdef"]}),
        ),
        (
            "send_marked_drafts",
            serde_json::json!({"marker_ids":["0123456789abcdef0123456789abcdef"]}),
        ),
    ] {
        let mut v = args;
        v["operation"] = operation.into();
        let request = serde_json::from_value::<BrokerRequest>(v.clone());
        assert!(request.is_ok(), "missing operation {operation}");
        assert_eq!(
            serde_json::to_value(request.unwrap().validate().unwrap()).unwrap(),
            v
        );
        v["account_id"] = "other".into();
        assert!(serde_json::from_value::<BrokerRequest>(v).is_err());
    }
}

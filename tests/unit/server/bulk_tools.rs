// SPDX-License-Identifier: MPL-2.0
use super::fixtures::{mcp_request, scripted_server, test_server};
use super::*;
fn cases() -> Vec<(
    &'static str,
    &'static str,
    usize,
    serde_json::Value,
    &'static str,
)> {
    vec![
        (
            "read_emails",
            "message_ids",
            20,
            serde_json::json!({"message_ids": ["m1"]}),
            "emails_read",
        ),
        (
            "apply_label_to_emails",
            "message_ids",
            100,
            serde_json::json!({"message_ids": ["m1"], "label_id": "Label_1"}),
            "labels_applied",
        ),
        (
            "mark_emails_read",
            "message_ids",
            100,
            serde_json::json!({"message_ids": ["m1"]}),
            "messages_read_state",
        ),
        (
            "mark_emails_unread",
            "message_ids",
            100,
            serde_json::json!({"message_ids": ["m1"]}),
            "messages_read_state",
        ),
        (
            "mark_emails_for_deletion",
            "message_ids",
            100,
            serde_json::json!({"message_ids": ["m1"]}),
            "emails_deletion_marked",
        ),
        (
            "delete_marked_emails",
            "marker_ids",
            100,
            serde_json::json!({"marker_ids": ["0123456789abcdef0123456789abcdef"]}),
            "emails_trashed",
        ),
        (
            "create_labels",
            "names",
            20,
            serde_json::json!({"names": ["Test"]}),
            "labels_created",
        ),
        (
            "delete_labels",
            "label_ids",
            20,
            serde_json::json!({"label_ids": ["Label_1"]}),
            "labels_deleted",
        ),
        (
            "create_drafts",
            "drafts",
            10,
            serde_json::json!({"drafts": [{"to": "test@example.com", "subject": "Test", "body": "Body"}]}),
            "drafts_created",
        ),
        (
            "create_reply_drafts",
            "replies",
            10,
            serde_json::json!({"replies": [{"message_id": "m1", "body": "Reply"}]}),
            "drafts_created",
        ),
        (
            "mark_drafts_for_deletion",
            "draft_ids",
            20,
            serde_json::json!({"draft_ids": ["d1"]}),
            "drafts_deletion_marked",
        ),
        (
            "mark_drafts_for_sending",
            "draft_ids",
            20,
            serde_json::json!({"draft_ids": ["d1"]}),
            "drafts_sending_marked",
        ),
        (
            "delete_marked_drafts",
            "marker_ids",
            20,
            serde_json::json!({"marker_ids": ["0123456789abcdef0123456789abcdef"]}),
            "drafts_deleted",
        ),
        (
            "send_marked_drafts",
            "marker_ids",
            20,
            serde_json::json!({"marker_ids": ["0123456789abcdef0123456789abcdef"]}),
            "drafts_sent",
        ),
    ]
}
#[tokio::test]
async fn bulk_tools_advertise_all_closed_schemas() {
    let (address, cancellation) = test_server().await;
    let response = mcp_request(address, "tools/list", serde_json::json!({})).await;
    let tools = response["result"]["tools"].as_array().unwrap();
    for (name, field, limit, _, _) in cases() {
        let tool = tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        let schema = &tool["inputSchema"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"][field]["minItems"], 1);
        assert_eq!(schema["properties"][field]["maxItems"], limit);
        for forbidden in [
            "account_id",
            "email",
            "query",
            "thread_id",
            "confirmed",
            "action",
        ] {
            assert!(schema["properties"].get(forbidden).is_none());
        }
    }
    cancellation.cancel();
}
#[cfg(unix)]
#[tokio::test]
async fn bulk_tools_forward_one_exact_typed_request_each() {
    let cases = cases();
    let script=cases.iter().map(|(name,_,_,args,status)|{let mut v=args.clone();v["operation"]=(*name).into();(serde_json::from_value::<BrokerRequest>(v).unwrap(),serde_json::from_value::<BrokerResponse>(serde_json::json!({"status":status,"result":{"items":[{"index":0,"status":"failed","code":"gmail_rate_limited","message":"Fixture"}]}})).unwrap())}).collect();
    let (address, cancellation, worker, path) = scripted_server(script).await;
    for (name, _, _, args, _) in cases {
        let result = mcp_request(
            address,
            "tools/call",
            serde_json::json!({"name":name,"arguments":args}),
        )
        .await;
        assert_eq!(
            result["result"]["structuredContent"],
            serde_json::json!({"items":[{"index":0,"status":"failed","code":"gmail_rate_limited","message":"Fixture"}]}),
            "{name}: {result}"
        );
    }
    cancellation.cancel();
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn bulk_tools_reject_selectors_and_empty_sets_before_broker_access() {
    let (address, cancellation) = test_server().await;
    for (name, field, _, args, _) in cases() {
        for malformed in [serde_json::json!({field:[]}), {
            let mut v = args;
            v["account_id"] = "other".into();
            v
        }] {
            let result = mcp_request(
                address,
                "tools/call",
                serde_json::json!({"name":name,"arguments":malformed}),
            )
            .await;
            assert!(
                result.get("error").is_some() || result["result"]["isError"] == true,
                "{result}"
            );
            assert!(
                !result.to_string().contains("connection response"),
                "must reject without broker access"
            );
        }
    }
    cancellation.cancel();
}

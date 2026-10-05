// SPDX-License-Identifier: MPL-2.0
use super::fixtures::{mcp_request, test_server};
use super::*;

#[tokio::test]
async fn all_emitted_schemas_have_exact_inputs_and_closed_objects() {
    let (address, cancellation) = test_server().await;
    let listed = mcp_request(address, "tools/list", serde_json::json!({})).await;
    let tools = listed["result"]["tools"].as_array().unwrap();
    let contracts: [(&str, &[&str], &[&str]); 17] = [
        (
            "list_emails",
            &[
                "query",
                "label_ids",
                "max_results",
                "page_token",
                "include_spam_trash",
            ],
            &[],
        ),
        ("list_labels", &[], &[]),
        ("create_label", &["name"], &["name"]),
        ("delete_label", &["label_id"], &["label_id"]),
        (
            "apply_label",
            &["message_id", "label_id"],
            &["message_id", "label_id"],
        ),
        ("read_email", &["message_id"], &["message_id"]),
        ("mark_email_read", &["message_id"], &["message_id"]),
        ("mark_email_unread", &["message_id"], &["message_id"]),
        ("mark_email_for_deletion", &["message_id"], &["message_id"]),
        ("delete_marked_email", &["marker_id"], &["marker_id"]),
        ("list_drafts", &["max_results", "page_token"], &[]),
        (
            "create_draft",
            &["to", "subject", "body"],
            &["to", "subject", "body"],
        ),
        (
            "create_reply_draft",
            &["message_id", "body"],
            &["message_id", "body"],
        ),
        ("mark_draft_for_deletion", &["draft_id"], &["draft_id"]),
        ("delete_marked_draft", &["marker_id"], &["marker_id"]),
        ("mark_draft_for_sending", &["draft_id"], &["draft_id"]),
        ("send_marked_draft", &["marker_id"], &["marker_id"]),
    ];
    assert_eq!(tools.len(), contracts.len() + 14);
    let bulk_names = [
        "read_emails",
        "apply_label_to_emails",
        "mark_emails_read",
        "mark_emails_unread",
        "mark_emails_for_deletion",
        "delete_marked_emails",
        "create_labels",
        "delete_labels",
        "create_drafts",
        "create_reply_drafts",
        "mark_drafts_for_deletion",
        "mark_drafts_for_sending",
        "delete_marked_drafts",
        "send_marked_drafts",
    ];
    for name in bulk_names {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{name}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{name}");
    }
    for (name, inputs, required) in contracts {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "{name}");
        assert_eq!(schema["additionalProperties"], false, "{name}");
        let mut keys: Vec<_> = schema["properties"]
            .as_object()
            .map(|props| props.keys().map(String::as_str).collect())
            .unwrap_or_default();
        keys.sort_unstable();
        let mut expected = inputs.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "{name}");
        let mut actual_required: Vec<_> = schema["required"]
            .as_array()
            .map(|fields| fields.iter().map(|field| field.as_str().unwrap()).collect())
            .unwrap_or_default();
        actual_required.sort_unstable();
        let mut expected_required = required.to_vec();
        expected_required.sort_unstable();
        assert_eq!(actual_required, expected_required, "{name}");
        let output = &tool["outputSchema"];
        assert_eq!(output["type"], "object", "{name}");
        let output_fields: &[&str] = match name {
            "list_emails" => &[
                "target_email",
                "messages",
                "next_page_token",
                "result_size_estimate",
            ],
            "list_labels" => &["labels"],
            "create_label" => &["id", "name", "type"],
            "delete_label" => &["label_id", "deleted"],
            "apply_label" => &["message_id", "label_id", "applied"],
            "read_email" => &[
                "message_id",
                "thread_id",
                "from",
                "recipients",
                "date",
                "subject",
                "labels",
                "body_text",
                "body_status",
            ],
            "mark_email_read" | "mark_email_unread" => &["message_id", "is_read"],
            "mark_email_for_deletion" => &["marker_id", "message_id", "expires_in_seconds"],
            "delete_marked_email" => &["message_id", "trashed"],
            "list_drafts" => &[
                "target_email",
                "drafts",
                "next_page_token",
                "result_size_estimate",
            ],
            "create_draft" | "create_reply_draft" | "send_marked_draft" => {
                &["draft_id", "message_id", "thread_id"]
            }
            "mark_draft_for_deletion" | "mark_draft_for_sending" => {
                &["marker_id", "draft_id", "expires_in_seconds"]
            }
            "delete_marked_draft" => &["draft_id", "deleted"],
            _ => unreachable!(),
        };
        let mut output_keys: Vec<_> = output["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        output_keys.sort_unstable();
        let mut expected_output = output_fields.to_vec();
        expected_output.sort_unstable();
        assert_eq!(output_keys, expected_output, "{name} output");
        for field in inputs {
            let property = &schema["properties"][field];
            match *field {
                "message_id" | "draft_id" => {
                    assert_eq!(property["type"], "string", "{name}.{field}");
                    assert_eq!(property["minLength"], 1);
                    assert_eq!(property["maxLength"], 256);
                    assert_eq!(property["pattern"], "^[A-Za-z0-9_-]+$");
                }
                "marker_id" => {
                    assert_eq!(property["type"], "string");
                    assert_eq!(property["minLength"], 32);
                    assert_eq!(property["maxLength"], 32);
                    assert_eq!(property["pattern"], "^[0-9a-f]{32}$");
                }
                "max_results" => {
                    assert!(schema_supports_type(property, "integer"));
                    assert_eq!(property["minimum"], 1);
                    assert_eq!(property["maximum"], 50);
                    assert_eq!(property["default"], 20);
                }
                "page_token" | "query" => {
                    assert!(schema_supports_type(property, "string"));
                    assert!(schema_supports_null(property));
                    assert_eq!(
                        property["maxLength"],
                        if *field == "query" { 1024 } else { 4096 }
                    );
                    assert_eq!(property["pattern"], r"^[^\u0000-\u001F\u007F-\u009F]*$");
                    if *field == "page_token" {
                        assert_eq!(property["minLength"], 1);
                        assert_eq!(property["default"], serde_json::Value::Null);
                    } else {
                        assert_eq!(property["default"], "in:inbox");
                    }
                }
                "label_ids" => {
                    assert_eq!(property["type"], "array");
                    assert_eq!(property["maxItems"], 20);
                    assert_eq!(property["default"], serde_json::json!([]));
                    assert_eq!(property["items"]["minLength"], 1);
                    assert_eq!(property["items"]["maxLength"], 256);
                    assert!(property["items"]["pattern"].is_string());
                }
                "include_spam_trash" => {
                    assert_eq!(property["type"], "boolean");
                    assert_eq!(property["default"], false);
                }
                "name" | "label_id" => {
                    assert_eq!(property["type"], "string");
                    assert_eq!(property["minLength"], 1);
                    assert!(property["pattern"].is_string());
                }
                "to" | "subject" | "body" => {
                    assert_eq!(property["type"], "string");
                    assert_eq!(property["minLength"], if *field == "to" { 3 } else { 1 });
                    assert_eq!(
                        property["maxLength"],
                        match *field {
                            "to" => 320,
                            "subject" => 998,
                            _ => 24_576,
                        }
                    );
                }
                _ => panic!("unaudited property {name}.{field}"),
            }
        }
        let rejected = mcp_request(
            address,
            "tools/call",
            serde_json::json!({"name":name,"arguments":{"account_id":"other"}}),
        )
        .await;
        assert_eq!(rejected["result"]["isError"], true, "{name}: {rejected}");
        let text = rejected["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("unknown field") || text.contains("missing field"),
            "{name}: {text}"
        );
    }
    cancellation.cancel();
}

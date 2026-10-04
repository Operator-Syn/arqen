// SPDX-License-Identifier: MPL-2.0
use super::fixtures::{mcp_request, test_server};
use super::*;

#[tokio::test]
async fn malformed_draft_ids_share_the_broker_invalid_request_category() {
    let (address, cancellation) = test_server().await;
    for name in ["mark_draft_for_deletion", "mark_draft_for_sending"] {
        for draft_id in [String::new(), "bad/id".into(), "a".repeat(257), "é".into()] {
            let broker: BrokerRequest =
                serde_json::from_value(serde_json::json!({"operation":name,"draft_id":draft_id}))
                    .unwrap();
            assert_eq!(
                broker.validate().unwrap_err().code.as_str(),
                "invalid_request"
            );
            let body = mcp_request(
                address,
                "tools/call",
                serde_json::json!({"name":name,"arguments":{"draft_id":draft_id}}),
            )
            .await;
            assert_eq!(body["result"]["isError"], true);
            assert_eq!(
                body["result"]["content"][0]["text"],
                "invalid_request: use a draft ID returned by list_drafts"
            );
        }
    }
    cancellation.cancel();
}

#[test]
fn textual_limits_count_unicode_scalars_not_utf8_bytes() {
    use arqen::gmail::{
        CreateDraftRequest, CreateReplyDraftRequest, ListDraftsRequest, ListEmailsRequest,
    };
    let draft =
        |to: String, subject: String, body: String| CreateDraftRequest { to, subject, body };
    let cases = [
        (
            "to",
            draft(format!("{}@x", "é".repeat(318)), "Hi".into(), "Body".into())
                .validate()
                .map(|_| ()),
        ),
        (
            "subject",
            draft("a@b".into(), "😀".repeat(998), "Body".into())
                .validate()
                .map(|_| ()),
        ),
        (
            "body",
            draft("a@b".into(), "Hi".into(), "😀".repeat(24_576))
                .validate()
                .map(|_| ()),
        ),
        (
            "reply body",
            CreateReplyDraftRequest {
                message_id: "source-1".into(),
                body: "😀".repeat(24_576),
            }
            .validate()
            .map(|_| ()),
        ),
        (
            "draft page token",
            ListDraftsRequest {
                page_token: Some("😀".repeat(4096)),
                ..Default::default()
            }
            .validate()
            .map(|_| ()),
        ),
        (
            "email text",
            ListEmailsRequest {
                query: Some("😀".repeat(1024)),
                label_ids: vec!["é".repeat(256)],
                page_token: Some("😀".repeat(4096)),
                ..Default::default()
            }
            .validate()
            .map(|_| ()),
        ),
    ];
    let failures: Vec<_> = cases
        .into_iter()
        .filter_map(|(name, result)| result.err().map(|error| format!("{name}: {error}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("; "));
    for (to, subject, body) in [
        (format!("{}@x", "é".repeat(319)), "Hi".into(), "Body".into()),
        ("a@b".into(), "😀".repeat(999), "Body".into()),
        ("a@b".into(), "Hi".into(), "😀".repeat(24_577)),
    ] {
        assert!(draft(to, subject, body).validate().is_err());
    }
    assert!(
        CreateReplyDraftRequest {
            message_id: "source-1".into(),
            body: "😀".repeat(24_577)
        }
        .validate()
        .is_err()
    );
    assert!(
        ListDraftsRequest {
            page_token: Some("😀".repeat(4097)),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}

#[tokio::test]
async fn emitted_reply_schema_has_the_runtime_ascii_message_id_bounds() {
    let (address, cancellation) = test_server().await;
    let listed = mcp_request(address, "tools/list", serde_json::json!({})).await;
    let tool = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "create_reply_draft")
        .unwrap();
    let id = &tool["inputSchema"]["properties"]["message_id"];
    assert_eq!(id["minLength"], 1);
    assert_eq!(id["maxLength"], 256);
    assert_eq!(id["pattern"], "^[A-Za-z0-9_-]+$");
    for message_id in [String::new(), "a".repeat(257), "é".into(), "bad/id".into()] {
        let body = mcp_request(address, "tools/call", serde_json::json!({"name":"create_reply_draft","arguments":{"message_id":message_id,"body":"Reply"}})).await;
        assert_eq!(body["result"]["isError"], true);
        assert_eq!(
            body["result"]["content"][0]["text"],
            "invalid_request: message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
        );
    }
    cancellation.cancel();
}

#[tokio::test]
async fn list_tools_reject_unknown_fields_in_emitted_schema_and_runtime() {
    let (address, cancellation) = test_server().await;
    let listed = mcp_request(address, "tools/list", serde_json::json!({})).await;
    for name in ["list_emails", "list_labels"] {
        let tool = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{name}");
        for field in ["account_id", "email", "unexpected"] {
            let body = mcp_request(
                address,
                "tools/call",
                serde_json::json!({"name":name,"arguments":{field:"other"}}),
            )
            .await;
            assert_eq!(body["result"]["isError"], true);
            assert!(
                body["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("unknown field"),
                "{name}: {body}"
            );
        }
    }
    cancellation.cancel();
}

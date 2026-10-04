// SPDX-License-Identifier: MPL-2.0
use super::fixtures::{mcp_request, scripted_server};
use super::*;
use arqen::gmail::*;

#[cfg(unix)]
#[tokio::test]
async fn list_outputs_feed_separate_apply_label_call_unchanged() {
    let messages = EmailListResponse {
        target_email: "selected@example.com".into(),
        messages: vec![EmailSummary {
            id: "message-1".into(),
            thread_id: "thread-1".into(),
            from: None,
            subject: Some("Subject".into()),
            date: None,
            labels: vec!["INBOX".into()],
            snippet: "Snippet".into(),
            snippet_truncated: false,
        }],
        next_page_token: None,
        result_size_estimate: Some(1),
    };
    let labels = LabelListResponse {
        labels: vec![EmailLabel {
            id: "Label_7".into(),
            name: "Project Atlas".into(),
            label_type: EmailLabelType::User,
        }],
    };
    let applied = LabelApplyResult {
        message_id: "message-1".into(),
        label_id: "Label_7".into(),
        applied: true,
    };
    let (address, cancellation, broker, socket_path) = scripted_server(vec![
        (
            BrokerRequest::ListEmails {
                request: ListEmailsRequest::default(),
            },
            BrokerResponse::Ok {
                result: messages.clone(),
            },
        ),
        (
            BrokerRequest::ListLabels,
            BrokerResponse::Labels {
                result: labels.clone(),
            },
        ),
        (
            BrokerRequest::ApplyLabel {
                request: ApplyLabelRequest {
                    message_id: "message-1".into(),
                    label_id: "Label_7".into(),
                },
            },
            BrokerResponse::LabelApplied {
                result: applied.clone(),
            },
        ),
    ])
    .await;
    let listed_messages = mcp_request(
        address,
        "tools/call",
        serde_json::json!({"name":"list_emails","arguments":{}}),
    )
    .await;
    assert_eq!(
        listed_messages["result"]["structuredContent"],
        serde_json::to_value(messages).unwrap()
    );
    let listed_labels = mcp_request(
        address,
        "tools/call",
        serde_json::json!({"name":"list_labels","arguments":{}}),
    )
    .await;
    assert_eq!(
        listed_labels["result"]["structuredContent"],
        serde_json::to_value(labels).unwrap()
    );
    let result = mcp_request(
        address,
        "tools/call",
        serde_json::json!({"name":"apply_label","arguments":{
            "message_id":listed_messages["result"]["structuredContent"]["messages"][0]["id"],
            "label_id":listed_labels["result"]["structuredContent"]["labels"][0]["id"]
        }}),
    )
    .await;
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(
        result["result"]["structuredContent"],
        serde_json::to_value(applied).unwrap()
    );
    broker.join().unwrap();
    cancellation.cancel();
    std::fs::remove_file(socket_path).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn message_mark_output_feeds_separate_trash_call_unchanged() {
    let mark = EmailDeletionMark {
        marker_id: "c".repeat(32),
        message_id: "message-1".into(),
        expires_in_seconds: 600,
    };
    let trashed = EmailTrashResult {
        message_id: "message-1".into(),
        trashed: true,
    };
    let (address, cancellation, broker, socket_path) = scripted_server(vec![
        (
            BrokerRequest::MarkEmailForDeletion {
                request: ReadEmailRequest {
                    message_id: "message-1".into(),
                },
            },
            BrokerResponse::DeletionMarked {
                result: mark.clone(),
            },
        ),
        (
            BrokerRequest::DeleteMarkedEmail {
                request: DeleteMarkedEmailRequest {
                    marker_id: mark.marker_id.clone(),
                },
            },
            BrokerResponse::EmailTrashed {
                result: trashed.clone(),
            },
        ),
    ])
    .await;
    let marked = mcp_request(address, "tools/call", serde_json::json!({"name":"mark_email_for_deletion","arguments":{"message_id":"message-1"}})).await;
    assert_eq!(marked["result"]["isError"], false);
    assert_eq!(
        marked["result"]["structuredContent"],
        serde_json::to_value(mark).unwrap()
    );
    let deleted = mcp_request(address, "tools/call", serde_json::json!({"name":"delete_marked_email","arguments":{"marker_id":marked["result"]["structuredContent"]["marker_id"]}})).await;
    assert_eq!(deleted["result"]["isError"], false);
    assert_eq!(
        deleted["result"]["structuredContent"],
        serde_json::to_value(trashed).unwrap()
    );
    broker.join().unwrap();
    cancellation.cancel();
    std::fs::remove_file(socket_path).unwrap();
}

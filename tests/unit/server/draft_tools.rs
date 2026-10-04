// SPDX-License-Identifier: MPL-2.0
use super::fixtures::{mcp_request, scripted_server};
use super::*;

#[cfg(unix)]
#[tokio::test]
async fn draft_tool_calls_forward_distinct_broker_operations_and_typed_results() {
    use arqen::gmail::*;
    let requests = [
        BrokerRequest::ListDrafts {
            request: ListDraftsRequest {
                max_results: 3,
                page_token: Some("next-😀".into()),
            },
        },
        BrokerRequest::CreateReplyDraft {
            request: CreateReplyDraftRequest {
                message_id: "source-1".into(),
                body: "Reply é".into(),
            },
        },
        BrokerRequest::CreateDraft {
            request: CreateDraftRequest {
                to: "person@example.com".into(),
                subject: "Hello é".into(),
                body: "Body\nline two".into(),
            },
        },
        BrokerRequest::MarkDraftForDeletion {
            request: DraftIdRequest {
                draft_id: "draft-1".into(),
            },
        },
        BrokerRequest::DeleteMarkedDraft {
            request: ActionMarkerRequest {
                marker_id: "a".repeat(32),
            },
        },
        BrokerRequest::MarkDraftForSending {
            request: DraftIdRequest {
                draft_id: "draft-1".into(),
            },
        },
        BrokerRequest::SendMarkedDraft {
            request: ActionMarkerRequest {
                marker_id: "b".repeat(32),
            },
        },
    ];
    let responses = [
        BrokerResponse::Drafts {
            result: arqen::gmail::DraftListResponse {
                target_email: "target@example.com".into(),
                drafts: vec![arqen::gmail::DraftSummary {
                    draft_id: "draft-1".into(),
                    message_id: "message-1".into(),
                    thread_id: "thread-1".into(),
                    to: vec!["recipient@example.com".into()],
                    subject: Some("Hello".into()),
                    date: None,
                    snippet: "Snippet é".into(),
                }],
                next_page_token: Some("following-page".into()),
                result_size_estimate: Some(1),
            },
        },
        BrokerResponse::DraftCreated {
            result: arqen::gmail::DraftCreateResult {
                draft_id: "reply-draft".into(),
                message_id: "reply-message".into(),
                thread_id: "thread-1".into(),
            },
        },
        BrokerResponse::DraftCreated {
            result: arqen::gmail::DraftCreateResult {
                draft_id: "new-draft".into(),
                message_id: "new-message".into(),
                thread_id: "new-thread".into(),
            },
        },
        BrokerResponse::DraftDeletionMarked {
            result: arqen::gmail::DraftActionMark {
                marker_id: "a".repeat(32),
                draft_id: "draft-1".into(),
                expires_in_seconds: 600,
            },
        },
        BrokerResponse::DraftDeleted {
            result: arqen::gmail::DraftDeleteResult {
                draft_id: "draft-1".into(),
                deleted: true,
            },
        },
        BrokerResponse::DraftSendingMarked {
            result: arqen::gmail::DraftActionMark {
                marker_id: "b".repeat(32),
                draft_id: "draft-1".into(),
                expires_in_seconds: 600,
            },
        },
        BrokerResponse::DraftSent {
            result: arqen::gmail::DraftSendResult {
                draft_id: "draft-1".into(),
                message_id: "sent-1".into(),
                thread_id: "thread-1".into(),
            },
        },
    ];
    let script = requests
        .iter()
        .cloned()
        .zip(responses.iter().cloned())
        .collect();
    let (address, cancellation, broker, socket_path) = scripted_server(script).await;
    let mut deletion_marker = None;
    let mut sending_marker = None;
    for (request, response) in requests.iter().zip(&responses) {
        let mut wire = serde_json::to_value(request).unwrap();
        let name = wire.as_object_mut().unwrap().remove("operation").unwrap();
        if name == "delete_marked_draft" {
            wire["marker_id"] = deletion_marker.take().unwrap();
        } else if name == "send_marked_draft" {
            wire["marker_id"] = sending_marker.take().unwrap();
        }
        let body = mcp_request(
            address,
            "tools/call",
            serde_json::json!({"name":name,"arguments":wire}),
        )
        .await;
        assert_eq!(body["result"]["isError"], false, "{name}: {body}");
        let expected = serde_json::to_value(response).unwrap()["result"].clone();
        assert_eq!(body["result"]["structuredContent"], expected, "{name}");
        if name == "mark_draft_for_deletion" {
            deletion_marker = Some(body["result"]["structuredContent"]["marker_id"].clone());
        } else if name == "mark_draft_for_sending" {
            sending_marker = Some(body["result"]["structuredContent"]["marker_id"].clone());
        }
    }
    broker.join().unwrap();
    cancellation.cancel();
    std::fs::remove_file(socket_path).unwrap();
}

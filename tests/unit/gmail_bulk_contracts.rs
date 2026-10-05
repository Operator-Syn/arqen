// SPDX-License-Identifier: MPL-2.0
use super::*;
#[test]
fn bulk_contract_reademailsrequest_enforces_limits() {
    let good = serde_json::json!("m1");
    for values in [vec![], vec![good.clone(); 21]] {
        let request: ReadEmailsRequest =
            serde_json::from_value(serde_json::json!({"message_ids":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: ReadEmailsRequest =
        serde_json::from_value(serde_json::json!({"message_ids":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<ReadEmailsRequest>(
            serde_json::json!({"message_ids":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(ReadEmailsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["message_ids"]["maxItems"], 20);
}
#[test]
fn bulk_contract_bulkemailmarkersrequest_enforces_limits() {
    let good = serde_json::json!("0123456789abcdef0123456789abcdef");
    for values in [vec![], vec![good.clone(); 101]] {
        let request: BulkEmailMarkersRequest =
            serde_json::from_value(serde_json::json!({"marker_ids":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: BulkEmailMarkersRequest =
        serde_json::from_value(serde_json::json!({"marker_ids":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<BulkEmailMarkersRequest>(
            serde_json::json!({"marker_ids":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(BulkEmailMarkersRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["marker_ids"]["maxItems"], 100);
}
#[test]
fn bulk_contract_createlabelsrequest_enforces_limits() {
    let good = serde_json::json!("m1");
    for values in [vec![], vec![good.clone(); 21]] {
        let request: CreateLabelsRequest =
            serde_json::from_value(serde_json::json!({"names":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: CreateLabelsRequest =
        serde_json::from_value(serde_json::json!({"names":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<CreateLabelsRequest>(
            serde_json::json!({"names":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(CreateLabelsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["names"]["maxItems"], 20);
}
#[test]
fn bulk_contract_deletelabelsrequest_enforces_limits() {
    let good = serde_json::json!("m1");
    for values in [vec![], vec![good.clone(); 21]] {
        let request: DeleteLabelsRequest =
            serde_json::from_value(serde_json::json!({"label_ids":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: DeleteLabelsRequest =
        serde_json::from_value(serde_json::json!({"label_ids":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<DeleteLabelsRequest>(
            serde_json::json!({"label_ids":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(DeleteLabelsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["label_ids"]["maxItems"], 20);
}
#[test]
fn bulk_contract_createdraftsrequest_enforces_limits() {
    let good = serde_json::json!({"to":"test@example.com","subject":"Test","body":"Body"});
    for values in [vec![], vec![good.clone(); 11]] {
        let request: CreateDraftsRequest =
            serde_json::from_value(serde_json::json!({"drafts":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: CreateDraftsRequest =
        serde_json::from_value(serde_json::json!({"drafts":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<CreateDraftsRequest>(
            serde_json::json!({"drafts":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(CreateDraftsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["drafts"]["maxItems"], 10);
}
#[test]
fn bulk_contract_createreplydraftsrequest_enforces_limits() {
    let good = serde_json::json!({"message_id":"m1","body":"Reply"});
    for values in [vec![], vec![good.clone(); 11]] {
        let request: CreateReplyDraftsRequest =
            serde_json::from_value(serde_json::json!({"replies":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: CreateReplyDraftsRequest =
        serde_json::from_value(serde_json::json!({"replies":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<CreateReplyDraftsRequest>(
            serde_json::json!({"replies":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(CreateReplyDraftsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["replies"]["maxItems"], 10);
}
#[test]
fn bulk_contract_bulkdraftidsrequest_enforces_limits() {
    let good = serde_json::json!("m1");
    for values in [vec![], vec![good.clone(); 21]] {
        let request: BulkDraftIdsRequest =
            serde_json::from_value(serde_json::json!({"draft_ids":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: BulkDraftIdsRequest =
        serde_json::from_value(serde_json::json!({"draft_ids":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<BulkDraftIdsRequest>(
            serde_json::json!({"draft_ids":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(BulkDraftIdsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["draft_ids"]["maxItems"], 20);
}
#[test]
fn bulk_contract_bulkdraftmarkersrequest_enforces_limits() {
    let good = serde_json::json!("0123456789abcdef0123456789abcdef");
    for values in [vec![], vec![good.clone(); 21]] {
        let request: BulkDraftMarkersRequest =
            serde_json::from_value(serde_json::json!({"marker_ids":values})).unwrap();
        assert!(request.validate().is_err());
    }
    let request: BulkDraftMarkersRequest =
        serde_json::from_value(serde_json::json!({"marker_ids":[good]})).unwrap();
    assert!(request.validate().is_ok());
    assert!(
        serde_json::from_value::<BulkDraftMarkersRequest>(
            serde_json::json!({"marker_ids":[],"account_id":"other"})
        )
        .is_err()
    );
    let schema = serde_json::to_value(schemars::schema_for!(BulkDraftMarkersRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["marker_ids"]["maxItems"], 20);
}
#[test]
fn bulk_contract_label_rejects_bad_ids() {
    assert!(
        ApplyLabelToEmailsRequest {
            message_ids: vec!["bad/id".into()],
            label_id: "Label_1".into()
        }
        .validate()
        .is_err()
    );
    assert!(
        ApplyLabelToEmailsRequest {
            message_ids: vec!["m1".into()],
            label_id: "".into()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn bulk_contract_outcomes_have_explicit_status() {
    let r = BulkResponse::<EmailReadState> {
        items: vec![BulkItem {
            index: 0,
            outcome: BulkOutcome::Unknown {
                code: "gmail_unavailable".into(),
                message: "Verify before retry".into(),
            },
        }],
    };
    let v = serde_json::to_value(r).unwrap();
    assert_eq!(v["items"][0]["status"], "unknown");
    assert!(v.get("success").is_none());
}

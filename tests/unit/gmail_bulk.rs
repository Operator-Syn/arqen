// SPDX-License-Identifier: MPL-2.0
use super::*;

#[test]
fn bulk_message_ids_reject_invalid_sets() {
    for message_ids in [
        vec![],
        vec!["m1".into(), "m1".into()],
        vec!["bad/id".into()],
        (0..101).map(|i| format!("m{i}")).collect(),
    ] {
        assert!(BulkMessageIdsRequest { message_ids }.validate().is_err());
    }
}

#[test]
fn bulk_message_ids_preserve_valid_boundary_sets() {
    for count in [1, 100] {
        let ids: Vec<String> = (0..count).rev().map(|i| format!("m{i}")).collect();
        assert_eq!(
            BulkMessageIdsRequest {
                message_ids: ids.clone()
            }
            .validate()
            .unwrap()
            .message_ids,
            ids
        );
    }
}

#[test]
fn bulk_message_ids_reject_unknown_fields_and_describe_bounds() {
    for field in [
        "account_id",
        "email",
        "query",
        "thread_id",
        "action",
        "confirmed",
    ] {
        let mut value = serde_json::json!({"message_ids": ["m1"]});
        value[field] = serde_json::json!("unexpected");
        assert!(serde_json::from_value::<BulkMessageIdsRequest>(value).is_err());
    }
    let schema = serde_json::to_value(schemars::schema_for!(BulkMessageIdsRequest)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["message_ids"]["minItems"], 1);
    assert_eq!(schema["properties"]["message_ids"]["maxItems"], 100);
    assert_eq!(
        schema["properties"]["message_ids"]["items"]["maxLength"],
        256
    );
}

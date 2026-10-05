// SPDX-License-Identifier: MPL-2.0
use super::*;
pub(super) fn create(
    request: CreateLabelsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let result = super::run_items_with_failure(
        &request.names,
        account,
        state,
        |api, token, name| {
            crate::gmail::bulk_write_outcome_as(
                || api.create_label(token, CreateLabelRequest { name: name.clone() }),
                |error| match super::super::map_create_label_error(error) {
                    BrokerResponse::Error { code, message } => BulkOutcome::Failed {
                        code: code.as_str().into(),
                        message,
                    },
                    _ => BulkOutcome::Failed {
                        code: "gmail_unavailable".into(),
                        message: "Gmail could not create the label".into(),
                    },
                },
            )
        },
        bulk_read_failure,
    );
    BrokerResponse::LabelsCreated { result }
}
pub(super) fn delete(
    request: DeleteLabelsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    // All label types are checked before the first DELETE; a system member aborts the set.
    let preflight = run_items(
        &request.label_ids,
        account,
        state,
        |api, token, id| match api.require_custom_label(token, id) {
            Ok(()) => Ok(BulkOutcome::Succeeded { result: () }),
            Err(e) if is_unauthorized(&e) => Err(e),
            Err(e) => Ok(bulk_read_failure(&e)),
        },
    );
    if preflight
        .items
        .iter()
        .any(|i| matches!(&i.outcome,BulkOutcome::Failed {code,..} if code=="system_label"))
    {
        return BrokerResponse::error(
            BrokerErrorCode::SystemLabel,
            "a system label was supplied; no labels were deleted",
        );
    }
    let entries: Vec<_> = request.label_ids.iter().zip(preflight.items).collect();
    let result = run_items(
        &entries,
        account,
        state,
        |api, token, (id, item)| match &item.outcome {
            BulkOutcome::Succeeded { .. } => {
                let outcome = bulk_write_outcome(|| {
                    api.delete_label(
                        token,
                        DeleteLabelRequest {
                            label_id: (*id).clone(),
                        },
                    )
                })?;
                if matches!(outcome, BulkOutcome::Unknown { .. })
                    && let Err(e) = api.require_custom_label(token, id)
                    && e.downcast_ref::<GmailApiError>()
                        .is_some_and(|e| e.status().as_u16() == 404)
                {
                    return Ok(BulkOutcome::Succeeded {
                        result: LabelDeleteResult {
                            label_id: (*id).clone(),
                            deleted: true,
                        },
                    });
                }
                Ok(outcome)
            }
            BulkOutcome::Failed { code, message } => Ok(BulkOutcome::Failed {
                code: if code == "message_not_found" {
                    "label_not_found".into()
                } else {
                    code.clone()
                },
                message: message.clone(),
            }),
            BulkOutcome::NotAttempted { code, message } => Ok(BulkOutcome::NotAttempted {
                code: code.clone(),
                message: message.clone(),
            }),
            BulkOutcome::Unknown { code, message } => Ok(BulkOutcome::Unknown {
                code: code.clone(),
                message: message.clone(),
            }),
        },
    );
    BrokerResponse::LabelsDeleted { result }
}

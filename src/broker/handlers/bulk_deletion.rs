// SPDX-License-Identifier: MPL-2.0
use super::*;
pub(super) fn mark(
    request: BulkMessageIdsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let resources: Vec<_> = request
        .message_ids
        .iter()
        .map(|id| ActionResource {
            message_id: id.clone(),
            draft_id: None,
        })
        .collect();
    let ids = match register_action_marks(
        state,
        &account.subject,
        &resources,
        PendingActionKind::TrashMessage,
    ) {
        Ok(ids) => ids,
        Err(e) => return action_error(e, true),
    };
    let items = ids
        .into_iter()
        .zip(request.message_ids)
        .enumerate()
        .map(|(index, (marker_id, message_id))| BulkItem {
            index,
            outcome: BulkOutcome::Succeeded {
                result: EmailDeletionMark {
                    marker_id,
                    message_id,
                    expires_in_seconds: DELETION_MARK_TTL.as_secs(),
                },
            },
        })
        .collect();
    BrokerResponse::EmailsDeletionMarked {
        result: BulkResponse { items },
    }
}
pub(super) fn execute(
    request: BulkEmailMarkersRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let marks = match consume_action_marks(
        state,
        &request.marker_ids,
        &account.subject,
        PendingActionKind::TrashMessage,
    ) {
        Ok(m) => m,
        Err(e) => return action_error(e, true),
    };
    let _finish = ActionFinishGuard {
        state,
        marks: &marks,
    };
    let result = run_items(&marks, account, state, |api, token, mark| {
        api.trash_email_verified(token, &mark.message_id)
    });
    BrokerResponse::EmailsTrashed { result }
}

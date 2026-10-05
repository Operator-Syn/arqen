// SPDX-License-Identifier: MPL-2.0
use super::*;
pub(super) fn create(
    request: CreateDraftsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let result = super::run_items(&request.drafts, account, state, |api, token, draft| {
        bulk_write_outcome(|| api.create_draft(token, draft.clone()))
    });
    BrokerResponse::DraftsCreated { result }
}
pub(super) fn reply(
    request: CreateReplyDraftsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let result = super::run_items(&request.replies, account, state, |api, token, reply| {
        bulk_write_outcome(|| api.create_reply_draft(token, reply.clone()))
    });
    BrokerResponse::DraftsCreated { result }
}
pub(super) fn mark(
    request: BulkDraftIdsRequest,
    kind: PendingActionKind,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let revisions = bounded_map(&request.draft_ids, |id| {
        draft_operation_with_refresh(account, state, |api, token| api.draft_message_id(token, id))
    });
    let mut resources = Vec::new();
    for (draft_id, result) in request.draft_ids.iter().zip(revisions) {
        match result {
            Ok(message_id) => resources.push(ActionResource {
                message_id,
                draft_id: Some(draft_id.clone()),
            }),
            Err(DraftOperationFailure::Credential(e)) => return map_credential_error(&e),
            Err(DraftOperationFailure::Gmail(e)) => {
                return super::super::drafts::map_draft_error(&e, "verify draft");
            }
        }
    }
    let markers = match register_action_marks(state, &account.subject, &resources, kind) {
        Ok(ids) => ids,
        Err(e) => return action_error(e, false),
    };
    let result = BulkResponse {
        items: markers
            .into_iter()
            .zip(request.draft_ids)
            .enumerate()
            .map(|(index, (marker_id, draft_id))| BulkItem {
                index,
                outcome: BulkOutcome::Succeeded {
                    result: DraftActionMark {
                        marker_id,
                        draft_id,
                        expires_in_seconds: DELETION_MARK_TTL.as_secs(),
                    },
                },
            })
            .collect(),
    };
    if kind == PendingActionKind::DeleteDraft {
        BrokerResponse::DraftsDeletionMarked { result }
    } else {
        BrokerResponse::DraftsSendingMarked { result }
    }
}
pub(super) fn delete(
    request: BulkDraftMarkersRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    execute(
        request,
        PendingActionKind::DeleteDraft,
        account,
        state,
        |id, _| DraftDeleteResult {
            draft_id: id.into(),
            deleted: true,
        },
        |result| BrokerResponse::DraftsDeleted { result },
    )
}
pub(super) fn send(
    request: BulkDraftMarkersRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    execute(
        request,
        PendingActionKind::SendDraft,
        account,
        state,
        |id, sent| {
            let sent = sent.expect("send execution returns a sent message");
            DraftSendResult {
                draft_id: id.into(),
                message_id: sent.message_id,
                thread_id: sent.thread_id,
            }
        },
        |result| BrokerResponse::DraftsSent { result },
    )
}
fn execute<T: Send>(
    request: BulkDraftMarkersRequest,
    kind: PendingActionKind,
    account: &Account,
    state: &BrokerState,
    convert: impl Fn(&str, Option<DraftCreateResult>) -> T + Sync,
    wrap: impl FnOnce(BulkResponse<T>) -> BrokerResponse,
) -> BrokerResponse {
    let marks = match consume_action_marks(state, &request.marker_ids, &account.subject, kind) {
        Ok(marks) => marks,
        Err(e) => return action_error(e, false),
    };
    let _finish = ActionFinishGuard {
        state,
        marks: &marks,
    };
    let result = run_items(&marks, account, state, |api, token, mark| {
        let Some(id) = mark.draft_id.as_deref() else {
            return Ok(BulkOutcome::Failed {
                code: "internal".into(),
                message: "incomplete draft marker".into(),
            });
        };
        crate::gmail::bulk_write_outcome_mapped(
            || {
                super::super::drafts::execute_draft_action(api, token, id, mark, kind)
                    .map(|sent| convert(id, sent))
            },
            |error| {
                outcome_from_response(
                    super::super::drafts::map_draft_error(
                        error,
                        if kind == PendingActionKind::SendDraft {
                            "send draft"
                        } else {
                            "delete draft"
                        },
                    ),
                    false,
                )
            },
        )
    });
    wrap(result)
}

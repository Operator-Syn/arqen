// SPDX-License-Identifier: MPL-2.0
use super::super::*;
use super::tokens::{DraftOperationFailure, draft_operation_with_refresh};

pub(in crate::broker) fn handle_list_drafts(
    request: crate::gmail::ListDraftsRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    match draft_operation_with_refresh(&account, state, |api, token| {
        api.list_drafts(token, &account.email, request.clone())
    }) {
        Ok(result) => BrokerResponse::Drafts { result },
        Err(DraftOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(DraftOperationFailure::Gmail(error)) => map_draft_error(&error, "list drafts"),
    }
}

pub(in crate::broker) fn handle_create_draft(
    request: crate::gmail::CreateDraftRequest,
    state: &BrokerState,
) -> BrokerResponse {
    create_draft_operation(request.validate(), state, |api, token, request| {
        api.create_draft(token, request)
    })
}

pub(in crate::broker) fn handle_create_reply_draft(
    request: crate::gmail::CreateReplyDraftRequest,
    state: &BrokerState,
) -> BrokerResponse {
    create_draft_operation(request.validate(), state, |api, token, request| {
        api.create_reply_draft(token, request)
    })
}

fn create_draft_operation<T, F>(
    request: anyhow::Result<T>,
    state: &BrokerState,
    operation: F,
) -> BrokerResponse
where
    T: Clone,
    F: Fn(&GmailApi, &str, T) -> anyhow::Result<crate::gmail::DraftCreateResult>,
{
    let request = match request {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidRequest,
                "draft content is invalid",
            );
        }
    };
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    if !has_gmail_modify_scope(&account) {
        return insufficient_scope_response();
    }
    match draft_operation_with_refresh(&account, state, |api, token| {
        operation(api, token, request.clone())
    }) {
        Ok(result) => BrokerResponse::DraftCreated { result },
        Err(DraftOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(DraftOperationFailure::Gmail(error)) => map_draft_error(&error, "create draft"),
    }
}

pub(in crate::broker) fn handle_mark_draft_for_deletion(
    request: crate::gmail::DraftIdRequest,
    state: &BrokerState,
) -> BrokerResponse {
    mark_draft_action(request, PendingActionKind::DeleteDraft, state)
}

pub(in crate::broker) fn handle_mark_draft_for_sending(
    request: crate::gmail::DraftIdRequest,
    state: &BrokerState,
) -> BrokerResponse {
    mark_draft_action(request, PendingActionKind::SendDraft, state)
}

fn mark_draft_action(
    request: crate::gmail::DraftIdRequest,
    kind: PendingActionKind,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(BrokerErrorCode::InvalidRequest, "draft_id is invalid");
        }
    };
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    if !has_gmail_modify_scope(&account) {
        return insufficient_scope_response();
    }
    let message_id = match draft_operation_with_refresh(&account, state, |api, token| {
        api.draft_message_id(token, &request.draft_id)
    }) {
        Ok(id) => id,
        Err(DraftOperationFailure::Credential(error)) => return map_credential_error(&error),
        Err(DraftOperationFailure::Gmail(error)) => return map_draft_error(&error, "verify draft"),
    };
    let marker_id = match register_action_mark(
        state,
        &account.subject,
        &message_id,
        Some(&request.draft_id),
        kind,
    ) {
        Ok(id) => id,
        Err(ActionMarkFailure::StoreUnavailable) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the action-mark store is unavailable",
            );
        }
        Err(ActionMarkFailure::Limit) => {
            return BrokerResponse::error(
                BrokerErrorCode::ActionMarkLimit,
                "too many pending action marks; complete an action or wait for marks to expire",
            );
        }
        Err(ActionMarkFailure::InProgress) => {
            return BrokerResponse::error(
                BrokerErrorCode::ActionInProgress,
                "an action is already executing for this draft",
            );
        }
        Err(ActionMarkFailure::Required) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the action-mark state is invalid",
            );
        }
    };
    let result = crate::gmail::DraftActionMark {
        marker_id,
        draft_id: request.draft_id,
        expires_in_seconds: DELETION_MARK_TTL.as_secs(),
    };
    if kind == PendingActionKind::DeleteDraft {
        BrokerResponse::DraftDeletionMarked { result }
    } else {
        BrokerResponse::DraftSendingMarked { result }
    }
}

pub(in crate::broker) fn handle_delete_marked_draft(
    request: crate::gmail::ActionMarkerRequest,
    state: &BrokerState,
) -> BrokerResponse {
    execute_marked_draft(request, PendingActionKind::DeleteDraft, state)
}

pub(in crate::broker) fn handle_send_marked_draft(
    request: crate::gmail::ActionMarkerRequest,
    state: &BrokerState,
) -> BrokerResponse {
    execute_marked_draft(request, PendingActionKind::SendDraft, state)
}

fn execute_marked_draft(
    request: crate::gmail::ActionMarkerRequest,
    kind: PendingActionKind,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidActionMarker,
                "marker_id is invalid",
            );
        }
    };
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    if !has_gmail_modify_scope(&account) {
        return insufficient_scope_response();
    }
    let mark = match consume_action_mark(state, &request.marker_id, &account.subject, kind) {
        Ok(mark) => mark,
        Err(ActionMarkFailure::StoreUnavailable) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the action-mark store is unavailable",
            );
        }
        Err(_) => return action_mark_required(kind),
    };
    let Some(draft_id) = mark.draft_id.clone() else {
        finish_action_mark(state, &mark);
        return BrokerResponse::error(
            BrokerErrorCode::Internal,
            "the draft action marker is incomplete",
        );
    };
    let result = draft_operation_with_refresh(&account, state, |api, token| {
        if kind == PendingActionKind::DeleteDraft {
            api.delete_draft(token, &draft_id).map(|_| None)
        } else {
            api.send_draft(token, &draft_id).map(Some)
        }
    });
    finish_action_mark(state, &mark);
    match result {
        Ok(None) => BrokerResponse::DraftDeleted {
            result: crate::gmail::DraftDeleteResult {
                draft_id,
                deleted: true,
            },
        },
        Ok(Some(result)) => BrokerResponse::DraftSent {
            result: crate::gmail::DraftSendResult {
                draft_id,
                message_id: result.message_id,
                thread_id: result.thread_id,
            },
        },
        Err(DraftOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(DraftOperationFailure::Gmail(error)) if kind == PendingActionKind::SendDraft => {
            map_send_draft_error(&error)
        }
        Err(DraftOperationFailure::Gmail(error)) => map_draft_error(&error, "delete draft"),
    }
}

fn action_mark_required(kind: PendingActionKind) -> BrokerResponse {
    let operation = if kind == PendingActionKind::SendDraft {
        "mark this exact draft for sending"
    } else {
        "mark this exact draft for deletion"
    };
    BrokerResponse::error(
        BrokerErrorCode::ActionMarkRequired,
        format!("{operation} first; markers expire after 10 minutes and can be used once"),
    )
}

fn map_draft_error(error: &anyhow::Error, operation: &str) -> BrokerResponse {
    if let Some(api_error) = error.downcast_ref::<GmailApiError>() {
        match api_error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    format!("Gmail rejected the {operation} request"),
                );
            }
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            404 => {
                let message = if operation == "create reply draft" {
                    "source message not found in the currently selected account; use a message_id from list_emails"
                } else {
                    "draft not found in the currently selected account; use a draft_id from list_drafts"
                };
                return BrokerResponse::error(BrokerErrorCode::MessageNotFound, message);
            }
            429 => {
                return BrokerResponse::error(
                    BrokerErrorCode::GmailRateLimited,
                    "Gmail is rate limiting requests; try again shortly",
                );
            }
            _ => (),
        }
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        format!("Gmail could not {operation}"),
    )
}

fn map_send_draft_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(api_error) = error.downcast_ref::<GmailApiError>() {
        match api_error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the draft-send request",
                );
            }
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            404 => {
                return BrokerResponse::error(
                    BrokerErrorCode::MessageNotFound,
                    "draft not found in the currently selected account; use a draft_id from list_drafts",
                );
            }
            429 => {
                return BrokerResponse::error(
                    BrokerErrorCode::GmailRateLimited,
                    "Gmail is rate limiting requests; try again shortly",
                );
            }
            _ => (),
        }
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not confirm whether the draft was sent; inspect list_drafts before retrying",
    )
}

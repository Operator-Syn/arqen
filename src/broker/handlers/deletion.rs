// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(in crate::broker) fn handle_mark_email_for_deletion(
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidMessageId,
                "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
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

    let marker_id = match register_action_mark(
        state,
        &account.subject,
        &request.message_id,
        None,
        PendingActionKind::TrashMessage,
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
                BrokerErrorCode::DeletionMarkLimit,
                "too many pending action marks; complete an action or wait for marks to expire",
            );
        }
        Err(ActionMarkFailure::InProgress) => {
            return BrokerResponse::error(
                BrokerErrorCode::ActionInProgress,
                "an action is already executing for this message",
            );
        }
        Err(ActionMarkFailure::Required) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the action-mark state is invalid",
            );
        }
    };
    BrokerResponse::DeletionMarked {
        result: crate::gmail::EmailDeletionMark {
            marker_id,
            message_id: request.message_id,
            expires_in_seconds: DELETION_MARK_TTL.as_secs(),
        },
    }
}

pub(in crate::broker) fn handle_delete_marked_email(
    request: crate::gmail::DeleteMarkedEmailRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidDeletionMark,
                "marker_id must be a 32-character deletion marker returned by mark_email_for_deletion",
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

    let mark = match consume_action_mark(
        state,
        &request.marker_id,
        &account.subject,
        PendingActionKind::TrashMessage,
    ) {
        Ok(mark) => mark,
        Err(ActionMarkFailure::Required) => return deletion_mark_required(),
        Err(ActionMarkFailure::StoreUnavailable) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the deletion-mark store is unavailable",
            );
        }
        Err(ActionMarkFailure::Limit | ActionMarkFailure::InProgress) => {
            return deletion_mark_required();
        }
    };
    let result = trash_email_with_refresh(
        &account,
        crate::gmail::ReadEmailRequest {
            message_id: mark.message_id.clone(),
        },
        state,
    );
    finish_action_mark(state, &mark);
    match result {
        Ok(result) => BrokerResponse::EmailTrashed { result },
        Err(TrashEmailFailure::Credential(error)) => map_credential_error(&error),
        Err(TrashEmailFailure::Gmail(error)) => map_trash_email_error(&error),
    }
}

fn deletion_mark_required() -> BrokerResponse {
    BrokerResponse::error(
        BrokerErrorCode::DeletionMarkRequired,
        "mark this exact message for deletion first; markers expire after 10 minutes and can be used once",
    )
}

fn map_trash_email_error(error: &anyhow::Error) -> BrokerResponse {
    if crate::gmail::is_draft_message_mutation(error) {
        return BrokerResponse::error(
            BrokerErrorCode::InvalidRequest,
            "draft messages must use mark_draft_for_deletion and delete_marked_draft",
        );
    }
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the message-trash request",
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
                    "Message not found in the currently selected account. Use an ID returned by list_emails.",
                );
            }
            429 => {
                return BrokerResponse::error(
                    BrokerErrorCode::GmailRateLimited,
                    "Gmail is rate limiting requests; try again shortly",
                );
            }
            _ => {}
        }
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not move the message to Trash",
    )
}

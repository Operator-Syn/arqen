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

    let now = Instant::now();
    let mut marks = match state.pending_deletions.lock() {
        Ok(marks) => marks,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the deletion-mark store is unavailable",
            );
        }
    };
    marks.retain(|_, mark| mark.expires_at > now);
    if marks.len() >= MAX_PENDING_DELETION_MARKS {
        return BrokerResponse::error(
            BrokerErrorCode::DeletionMarkLimit,
            "too many pending deletion marks; delete marked messages or wait for marks to expire",
        );
    }

    let marker_id = uuid::Uuid::new_v4().simple().to_string();
    marks.insert(
        marker_id.clone(),
        PendingDeletionMark {
            google_subject: account.subject,
            message_id: request.message_id.clone(),
            expires_at: now + DELETION_MARK_TTL,
        },
    );
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

    let message_id = match consume_deletion_mark(&request.marker_id, &account.subject, state) {
        Ok(message_id) => message_id,
        Err(DeletionMarkFailure::Required) => return deletion_mark_required(),
        Err(DeletionMarkFailure::StoreUnavailable) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the deletion-mark store is unavailable",
            );
        }
    };
    let message_request = crate::gmail::ReadEmailRequest { message_id };
    match trash_email_with_refresh(&account, message_request, state) {
        Ok(result) => BrokerResponse::EmailTrashed { result },
        Err(TrashEmailFailure::Credential(error)) => map_credential_error(&error),
        Err(TrashEmailFailure::Gmail(error)) => map_trash_email_error(&error),
    }
}

fn consume_deletion_mark(
    marker_id: &str,
    google_subject: &str,
    state: &BrokerState,
) -> std::result::Result<String, DeletionMarkFailure> {
    let mut marks = state
        .pending_deletions
        .lock()
        .map_err(|_| DeletionMarkFailure::StoreUnavailable)?;
    let Some(mark) = marks.get(marker_id) else {
        return Err(DeletionMarkFailure::Required);
    };
    if mark.expires_at <= Instant::now() {
        marks.remove(marker_id);
        return Err(DeletionMarkFailure::Required);
    }
    if mark.google_subject != google_subject {
        return Err(DeletionMarkFailure::Required);
    }
    marks
        .remove(marker_id)
        .map(|mark| mark.message_id)
        .ok_or(DeletionMarkFailure::Required)
}

#[derive(Debug, Clone, Copy)]
enum DeletionMarkFailure {
    Required,
    StoreUnavailable,
}

fn deletion_mark_required() -> BrokerResponse {
    BrokerResponse::error(
        BrokerErrorCode::DeletionMarkRequired,
        "mark this exact message for deletion first; markers expire after 10 minutes and can be used once",
    )
}

fn map_trash_email_error(error: &anyhow::Error) -> BrokerResponse {
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

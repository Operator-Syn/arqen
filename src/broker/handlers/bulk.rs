// SPDX-License-Identifier: MPL-2.0
use super::tokens::{DraftOperationFailure, cached_or_refresh_token, draft_operation_with_refresh};
use super::*;
use crate::gmail::concurrency::bounded_map;
use crate::gmail::*;
#[path = "bulk_deletion.rs"]
mod bulk_deletion;
#[path = "bulk_drafts.rs"]
mod bulk_drafts;
#[path = "bulk_emails.rs"]
mod bulk_emails;
#[path = "bulk_labels.rs"]
mod bulk_labels;
pub(in crate::broker) fn handle_bulk_request(
    request: BrokerRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(r) => r,
        Err(e) => return BrokerResponse::error(e.code, e.message),
    };
    let store = match AccountStore::open(&state.database_path) {
        Ok(s) => s,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    if !matches!(request, BrokerRequest::ReadEmails { .. }) && !has_gmail_modify_scope(&account) {
        return insufficient_scope_response();
    }
    if !matches!(request, BrokerRequest::MarkEmailsForDeletion { .. })
        && let Err(e) = cached_or_refresh_token(&account, state)
    {
        return map_credential_error(&e);
    }
    match request {
        BrokerRequest::ReadEmails { request } => bulk_emails::read(request, &account, state),
        BrokerRequest::ApplyLabelToEmails { request } => provider_response(
            &account,
            state,
            |api, token| api.apply_label_to_emails(token, request.clone()),
            |result| BrokerResponse::LabelsApplied { result },
        ),
        BrokerRequest::MarkEmailsRead { request } => provider_response(
            &account,
            state,
            |api, token| api.mark_emails_state(token, request.clone(), true),
            |result| BrokerResponse::MessagesReadState { result },
        ),
        BrokerRequest::MarkEmailsUnread { request } => provider_response(
            &account,
            state,
            |api, token| api.mark_emails_state(token, request.clone(), false),
            |result| BrokerResponse::MessagesReadState { result },
        ),
        BrokerRequest::MarkEmailsForDeletion { request } => {
            bulk_deletion::mark(request, &account, state)
        }
        BrokerRequest::DeleteMarkedEmails { request } => {
            bulk_deletion::execute(request, &account, state)
        }
        BrokerRequest::CreateLabels { request } => bulk_labels::create(request, &account, state),
        BrokerRequest::DeleteLabels { request } => bulk_labels::delete(request, &account, state),
        BrokerRequest::CreateDrafts { request } => bulk_drafts::create(request, &account, state),
        BrokerRequest::CreateReplyDrafts { request } => {
            bulk_drafts::reply(request, &account, state)
        }
        BrokerRequest::MarkDraftsForDeletion { request } => {
            bulk_drafts::mark(request, PendingActionKind::DeleteDraft, &account, state)
        }
        BrokerRequest::MarkDraftsForSending { request } => {
            bulk_drafts::mark(request, PendingActionKind::SendDraft, &account, state)
        }
        BrokerRequest::DeleteMarkedDrafts { request } => {
            bulk_drafts::delete(request, &account, state)
        }
        BrokerRequest::SendMarkedDrafts { request } => bulk_drafts::send(request, &account, state),
        _ => BrokerResponse::error(BrokerErrorCode::InvalidRequest, "not a bulk operation"),
    }
}
fn provider_response<T>(
    account: &Account,
    state: &BrokerState,
    operation: impl Fn(&GmailApi, &str) -> Result<T>,
    wrap: impl FnOnce(T) -> BrokerResponse,
) -> BrokerResponse {
    match draft_operation_with_refresh(account, state, operation) {
        Ok(result) => wrap(result),
        Err(DraftOperationFailure::Credential(e)) => map_credential_error(&e),
        Err(DraftOperationFailure::Gmail(e)) => map_delete_label_error(&e),
    }
}
fn run_items<I: Sync, T: Send>(
    inputs: &[I],
    account: &Account,
    state: &BrokerState,
    operation: impl Fn(&GmailApi, &str, &I) -> Result<BulkOutcome<T>> + Sync,
) -> BulkResponse<T> {
    run_items_with_failure(inputs, account, state, operation, bulk_read_failure)
}
fn run_items_with_failure<I: Sync, T: Send>(
    inputs: &[I],
    account: &Account,
    state: &BrokerState,
    operation: impl Fn(&GmailApi, &str, &I) -> Result<BulkOutcome<T>> + Sync,
    failure: impl Fn(&anyhow::Error) -> BulkOutcome<T> + Sync,
) -> BulkResponse<T> {
    let fatal = AtomicBool::new(false);
    let outcomes = bounded_map(inputs, |input| {
        if fatal.load(Ordering::Acquire) {
            return BulkOutcome::NotAttempted {
                code: "credential_unavailable".into(),
                message: "credential failure stopped further dispatch".into(),
            };
        }
        match draft_operation_with_refresh(account, state, |api, token| {
            operation(api, token, input)
        }) {
            Ok(outcome) => outcome,
            Err(DraftOperationFailure::Credential(e)) => {
                fatal.store(true, Ordering::Release);
                outcome_from_response(map_credential_error(&e), true)
            }
            Err(DraftOperationFailure::Gmail(e)) => failure(&e),
        }
    });
    BulkResponse {
        items: outcomes
            .into_iter()
            .enumerate()
            .map(|(index, outcome)| BulkItem { index, outcome })
            .collect(),
    }
}
fn outcome_from_response<T>(response: BrokerResponse, not_attempted: bool) -> BulkOutcome<T> {
    let (code, message) = match response {
        BrokerResponse::Error { code, message } => (code.as_str().into(), message),
        _ => ("internal".into(), "invalid failure mapping".into()),
    };
    if not_attempted {
        BulkOutcome::NotAttempted { code, message }
    } else {
        BulkOutcome::Failed { code, message }
    }
}
fn action_error(error: ActionMarkFailure, email: bool) -> BrokerResponse {
    let code = match error {
        ActionMarkFailure::StoreUnavailable => BrokerErrorCode::Internal,
        ActionMarkFailure::Limit => {
            if email {
                BrokerErrorCode::DeletionMarkLimit
            } else {
                BrokerErrorCode::ActionMarkLimit
            }
        }
        ActionMarkFailure::InProgress => BrokerErrorCode::ActionInProgress,
        ActionMarkFailure::Required => {
            if email {
                BrokerErrorCode::DeletionMarkRequired
            } else {
                BrokerErrorCode::ActionMarkRequired
            }
        }
    };
    BrokerResponse::error(
        code,
        "the complete action set could not be admitted; no marks were registered or consumed",
    )
}

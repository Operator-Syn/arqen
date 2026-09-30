// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(in crate::broker) fn handle_list_emails(
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database is unavailable",
            );
        }
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    match list_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::Ok { result },
        Err(ListEmailsFailure::Credential(error)) => map_credential_error(&error),
        Err(ListEmailsFailure::Gmail(error)) => map_gmail_error(&error),
    }
}

pub(in crate::broker) fn handle_read_email(
    request: crate::gmail::ReadEmailRequest,
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
    match read_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::ReadEmail { result },
        Err(ReadEmailFailure::Credential(error)) => map_credential_error(&error),
        Err(ReadEmailFailure::Gmail(error)) => map_read_email_error(&error),
    }
}

pub(in crate::broker) fn handle_mark_email_read(
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> BrokerResponse {
    handle_mark_email_state(request, true, state)
}

pub(in crate::broker) fn handle_mark_email_unread(
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> BrokerResponse {
    handle_mark_email_state(request, false, state)
}

fn handle_mark_email_state(
    request: crate::gmail::ReadEmailRequest,
    is_read: bool,
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
    if !has_gmail_modify_scope(&account) {
        return insufficient_scope_response();
    }
    match mark_email_with_refresh(&account, request, is_read, state) {
        Ok(result) => BrokerResponse::MessageReadState { result },
        Err(MarkEmailFailure::Credential(error)) => map_credential_error(&error),
        Err(MarkEmailFailure::Gmail(error)) => map_mark_email_error(&error),
    }
}

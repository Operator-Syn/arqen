// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(in crate::broker) fn handle_list_labels(state: &BrokerState) -> BrokerResponse {
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
    match list_labels_with_refresh(&account, state) {
        Ok(result) => BrokerResponse::Labels { result },
        Err(ListLabelsFailure::Credential(error)) => map_credential_error(&error),
        Err(ListLabelsFailure::Gmail(error)) => map_gmail_error(&error),
    }
}

pub(in crate::broker) fn handle_create_label(
    request: crate::gmail::CreateLabelRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidLabelName,
                "name must be nonblank and contain no control characters",
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
    match create_label_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::LabelCreated { result },
        Err(LabelOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(LabelOperationFailure::Gmail(error)) => map_create_label_error(&error),
    }
}

pub(in crate::broker) fn handle_delete_label(
    request: crate::gmail::DeleteLabelRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let request = match request.validate() {
        Ok(request) => request,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::InvalidLabelId,
                "label_id must be nonempty and contain no control characters",
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
    match delete_label_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::LabelDeleted { result },
        Err(LabelOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(LabelOperationFailure::Gmail(error)) => map_delete_label_error(&error),
    }
}

pub(in crate::broker) fn handle_apply_label(
    request: crate::gmail::ApplyLabelRequest,
    state: &BrokerState,
) -> BrokerResponse {
    if request.validate_message_id().is_err() {
        return BrokerResponse::error(
            BrokerErrorCode::InvalidMessageId,
            "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
        );
    }
    if request.validate_label_id().is_err() {
        return BrokerResponse::error(
            BrokerErrorCode::InvalidLabelId,
            "label_id must be nonempty and contain no control characters",
        );
    }
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
    match apply_label_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::LabelApplied { result },
        Err(ApplyLabelOperationFailure::Credential(error)) => map_credential_error(&error),
        Err(ApplyLabelOperationFailure::Gmail(error)) => map_apply_label_error(error),
    }
}

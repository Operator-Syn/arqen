// SPDX-License-Identifier: MPL-2.0
use super::super::*;

pub(crate) fn map_gmail_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
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
        return BrokerResponse::error(
            BrokerErrorCode::GmailUnavailable,
            "Gmail could not complete the mail-list request",
        );
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not complete the request",
    )
}

pub(crate) fn map_read_email_error(error: &anyhow::Error) -> BrokerResponse {
    if is_read_email_too_large(error) {
        return BrokerResponse::error(
            BrokerErrorCode::MessageTooLarge,
            "the message exceeds the 256 KiB readable-body limit",
        );
    }
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the message-read request",
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
        "Gmail could not complete the message-read request",
    )
}

pub(crate) fn map_mark_email_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the message-state request",
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
        "Gmail could not update the message read state",
    )
}

pub(crate) fn map_create_label_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidLabelName,
                    "Gmail rejected the label name; it may already exist or conflict with a reserved system label",
                );
            }
            409 => {
                return BrokerResponse::error(
                    BrokerErrorCode::LabelAlreadyExists,
                    "a label with this name already exists in the selected account",
                );
            }
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
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
        "Gmail could not create the label",
    )
}

pub(crate) fn map_delete_label_error(error: &anyhow::Error) -> BrokerResponse {
    if error
        .downcast_ref::<crate::gmail::SystemLabelError>()
        .is_some()
    {
        return BrokerResponse::error(
            BrokerErrorCode::SystemLabel,
            "Gmail system labels cannot be deleted; choose a label with type user",
        );
    }
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidLabelId,
                    "Gmail rejected the label ID; use the exact ID returned by list_labels",
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
                    BrokerErrorCode::LabelNotFound,
                    "Label not found in the currently selected account. Use an ID returned by list_labels.",
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
        "Gmail could not delete the label",
    )
}

pub(crate) fn map_apply_label_error(error: crate::gmail::ApplyLabelFailure) -> BrokerResponse {
    use crate::gmail::ApplyLabelFailure;

    match error {
        ApplyLabelFailure::InvalidMessageId => BrokerResponse::error(
            BrokerErrorCode::InvalidMessageId,
            "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
        ),
        ApplyLabelFailure::InvalidLabelId => BrokerResponse::error(
            BrokerErrorCode::InvalidLabelId,
            "label_id must be nonempty and contain no control characters",
        ),
        ApplyLabelFailure::SystemLabel => BrokerResponse::error(
            BrokerErrorCode::SystemLabel,
            "apply_label only supports custom user labels; system labels are rejected",
        ),
        ApplyLabelFailure::LabelLookup(error) => {
            if error
                .downcast_ref::<crate::gmail::LabelNotFoundError>()
                .is_some()
            {
                return BrokerResponse::error(
                    BrokerErrorCode::LabelNotFound,
                    "label not found in the selected account; use an ID returned by list_labels",
                );
            }
            map_apply_label_provider_error(&error, true)
        }
        ApplyLabelFailure::MessageModify(error) => map_apply_label_provider_error(&error, false),
    }
}

pub(crate) fn map_apply_label_provider_error(
    error: &anyhow::Error,
    checking_label: bool,
) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 if checking_label => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidLabelId,
                    "Gmail rejected the label ID; use a custom label ID returned by list_labels",
                );
            }
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the message-label request",
                );
            }
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            404 if checking_label => {
                return BrokerResponse::error(
                    BrokerErrorCode::LabelNotFound,
                    "label not found in the selected account; use an ID returned by list_labels",
                );
            }
            404 => {
                return BrokerResponse::error(
                    BrokerErrorCode::MessageNotFound,
                    "message not found in the selected account; use an ID returned by list_emails",
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
        if checking_label {
            "Gmail could not verify the label"
        } else {
            "Gmail could not apply the label to the message"
        },
    )
}

pub(crate) fn map_credential_error(error: &anyhow::Error) -> BrokerResponse {
    if is_invalid_grant(error) || is_missing_refresh_token(error) {
        return BrokerResponse::error(
            BrokerErrorCode::ReauthenticationRequired,
            "reauthenticate the selected MCP target account in Arqen",
        );
    }
    BrokerResponse::error(
        BrokerErrorCode::CredentialUnavailable,
        "the selected account credential is temporarily unavailable",
    )
}

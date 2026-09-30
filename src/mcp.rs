// SPDX-License-Identifier: MPL-2.0
use crate::gmail::{
    ActionMarkerRequest, ApplyLabelRequest, CreateDraftRequest, CreateLabelRequest,
    CreateReplyDraftRequest, DeleteLabelRequest, DeleteMarkedEmailRequest, DraftActionMark,
    DraftCreateResult, DraftDeleteResult, DraftIdRequest, DraftListResponse, DraftSendResult,
    EmailDeletionMark, EmailLabel, EmailListResponse, EmailReadResponse, EmailReadState,
    EmailTrashResult, LabelApplyResult, LabelDeleteResult, LabelListResponse, ListDraftsRequest,
    ListEmailsRequest, ReadEmailRequest,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrokerRequest {
    ListEmails {
        #[serde(flatten)]
        request: ListEmailsRequest,
    },
    ListLabels,
    CreateLabel {
        #[serde(flatten)]
        request: CreateLabelRequest,
    },
    DeleteLabel {
        #[serde(flatten)]
        request: DeleteLabelRequest,
    },
    ApplyLabel {
        #[serde(flatten)]
        request: ApplyLabelRequest,
    },
    ReadEmail {
        #[serde(flatten)]
        request: ReadEmailRequest,
    },
    MarkEmailRead {
        #[serde(flatten)]
        request: ReadEmailRequest,
    },
    MarkEmailUnread {
        #[serde(flatten)]
        request: ReadEmailRequest,
    },
    MarkEmailForDeletion {
        #[serde(flatten)]
        request: ReadEmailRequest,
    },
    DeleteMarkedEmail {
        #[serde(flatten)]
        request: DeleteMarkedEmailRequest,
    },
    ListDrafts {
        #[serde(flatten)]
        request: ListDraftsRequest,
    },
    CreateDraft {
        #[serde(flatten)]
        request: CreateDraftRequest,
    },
    CreateReplyDraft {
        #[serde(flatten)]
        request: CreateReplyDraftRequest,
    },
    MarkDraftForDeletion {
        #[serde(flatten)]
        request: DraftIdRequest,
    },
    DeleteMarkedDraft {
        #[serde(flatten)]
        request: ActionMarkerRequest,
    },
    MarkDraftForSending {
        #[serde(flatten)]
        request: DraftIdRequest,
    },
    SendMarkedDraft {
        #[serde(flatten)]
        request: ActionMarkerRequest,
    },
    Readiness {
        #[serde(flatten)]
        request: ListEmailsRequest,
    },
}

impl BrokerRequest {
    pub fn list_emails(request: ListEmailsRequest) -> Self {
        Self::ListEmails { request }
    }

    pub fn list_labels() -> Self {
        Self::ListLabels
    }

    pub fn create_label(request: CreateLabelRequest) -> Self {
        Self::CreateLabel { request }
    }

    pub fn delete_label(request: DeleteLabelRequest) -> Self {
        Self::DeleteLabel { request }
    }

    pub fn apply_label(request: ApplyLabelRequest) -> Self {
        Self::ApplyLabel { request }
    }

    pub fn read_email(request: ReadEmailRequest) -> Self {
        Self::ReadEmail { request }
    }

    pub fn mark_email_read(request: ReadEmailRequest) -> Self {
        Self::MarkEmailRead { request }
    }

    pub fn mark_email_unread(request: ReadEmailRequest) -> Self {
        Self::MarkEmailUnread { request }
    }

    pub fn mark_email_for_deletion(request: ReadEmailRequest) -> Self {
        Self::MarkEmailForDeletion { request }
    }

    pub fn delete_marked_email(request: DeleteMarkedEmailRequest) -> Self {
        Self::DeleteMarkedEmail { request }
    }

    pub fn list_drafts(request: ListDraftsRequest) -> Self {
        Self::ListDrafts { request }
    }
    pub fn create_draft(request: CreateDraftRequest) -> Self {
        Self::CreateDraft { request }
    }
    pub fn create_reply_draft(request: CreateReplyDraftRequest) -> Self {
        Self::CreateReplyDraft { request }
    }
    pub fn mark_draft_for_deletion(request: DraftIdRequest) -> Self {
        Self::MarkDraftForDeletion { request }
    }
    pub fn delete_marked_draft(request: ActionMarkerRequest) -> Self {
        Self::DeleteMarkedDraft { request }
    }
    pub fn mark_draft_for_sending(request: DraftIdRequest) -> Self {
        Self::MarkDraftForSending { request }
    }
    pub fn send_marked_draft(request: ActionMarkerRequest) -> Self {
        Self::SendMarkedDraft { request }
    }

    pub fn readiness() -> Self {
        Self::Readiness {
            request: ListEmailsRequest::default(),
        }
    }

    pub fn validate(self) -> std::result::Result<Self, BrokerValidationFailure> {
        match self {
            Self::ListEmails { request } => request
                .validate()
                .map(|request| Self::ListEmails { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the mail-list request is invalid",
                }),
            Self::ListLabels => Ok(Self::ListLabels),
            Self::CreateLabel { request } => request
                .validate()
                .map(|request| Self::CreateLabel { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidLabelName,
                    message: "name must be nonblank and contain no control characters",
                }),
            Self::DeleteLabel { request } => request
                .validate()
                .map(|request| Self::DeleteLabel { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidLabelId,
                    message: "label_id must be nonempty and contain no control characters",
                }),
            Self::ApplyLabel { request } => {
                request.validate_message_id().map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
                })?;
                request.validate_label_id().map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidLabelId,
                    message: "label_id must be nonempty and contain no control characters",
                })?;
                Ok(Self::ApplyLabel { request })
            }
            Self::ReadEmail { request } => request
                .validate()
                .map(|request| Self::ReadEmail { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
                }),
            Self::MarkEmailRead { request } => request
                .validate()
                .map(|request| Self::MarkEmailRead { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
                }),
            Self::MarkEmailUnread { request } => request
                .validate()
                .map(|request| Self::MarkEmailUnread { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
                }),
            Self::MarkEmailForDeletion { request } => request
                .validate()
                .map(|request| Self::MarkEmailForDeletion { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores",
                }),
            Self::DeleteMarkedEmail { request } => request
                .validate()
                .map(|request| Self::DeleteMarkedEmail { request })
                .map_err(|_| BrokerValidationFailure {
                    code: BrokerErrorCode::InvalidDeletionMark,
                    message: "marker_id must be a 32-character deletion marker returned by mark_email_for_deletion",
                }),
            Self::ListDrafts { request } => request.validate().map(|request| Self::ListDrafts { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidRequest, message: "the draft-list request is invalid" }),
            Self::CreateDraft { request } => request.validate().map(|request| Self::CreateDraft { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidRequest, message: "the draft content is invalid" }),
            Self::CreateReplyDraft { request } => request.validate().map(|request| Self::CreateReplyDraft { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidRequest, message: "the reply-draft request is invalid" }),
            Self::MarkDraftForDeletion { request } => request.validate().map(|request| Self::MarkDraftForDeletion { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidRequest, message: "the draft ID is invalid" }),
            Self::MarkDraftForSending { request } => request.validate().map(|request| Self::MarkDraftForSending { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidRequest, message: "the draft ID is invalid" }),
            Self::DeleteMarkedDraft { request } => request.validate().map(|request| Self::DeleteMarkedDraft { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidActionMarker, message: "the action marker is invalid" }),
            Self::SendMarkedDraft { request } => request.validate().map(|request| Self::SendMarkedDraft { request }).map_err(|_| BrokerValidationFailure { code: BrokerErrorCode::InvalidActionMarker, message: "the action marker is invalid" }),
            Self::Readiness { request } => Ok(Self::Readiness { request }),
        }
    }

    pub fn validate_readiness(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(self, Self::Readiness { .. }),
            "unsupported broker operation"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerValidationFailure {
    pub code: BrokerErrorCode,
    pub message: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerErrorCode {
    InvalidRequest,
    InvalidMessageId,
    MessageNotFound,
    InsufficientScope,
    MessageTooLarge,
    TargetNotConfigured,
    TargetUnavailable,
    ReauthenticationRequired,
    CredentialUnavailable,
    GmailRateLimited,
    GmailUnavailable,
    DeletionMarkRequired,
    InvalidDeletionMark,
    DeletionMarkLimit,
    Internal,
    InvalidLabelName,
    LabelAlreadyExists,
    InvalidLabelId,
    LabelNotFound,
    SystemLabel,
    ActionInProgress,
    ActionMarkRequired,
    InvalidActionMarker,
    ActionMarkLimit,
}

impl BrokerErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidMessageId => "invalid_message_id",
            Self::MessageNotFound => "message_not_found",
            Self::InsufficientScope => "insufficient_scope",
            Self::MessageTooLarge => "message_too_large",
            Self::TargetNotConfigured => "target_not_configured",
            Self::TargetUnavailable => "target_unavailable",
            Self::ReauthenticationRequired => "reauthentication_required",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::GmailRateLimited => "gmail_rate_limited",
            Self::GmailUnavailable => "gmail_unavailable",
            Self::DeletionMarkRequired => "deletion_mark_required",
            Self::InvalidDeletionMark => "invalid_deletion_mark",
            Self::DeletionMarkLimit => "deletion_mark_limit",
            Self::Internal => "internal",
            Self::InvalidLabelName => "invalid_label_name",
            Self::LabelAlreadyExists => "label_already_exists",
            Self::InvalidLabelId => "invalid_label_id",
            Self::LabelNotFound => "label_not_found",
            Self::SystemLabel => "system_label",
            Self::ActionInProgress => "action_in_progress",
            Self::ActionMarkRequired => "action_mark_required",
            Self::InvalidActionMarker => "invalid_action_marker",
            Self::ActionMarkLimit => "action_mark_limit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerFailure {
    pub code: BrokerErrorCode,
    pub message: String,
}

impl std::fmt::Display for BrokerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for BrokerFailure {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BrokerResponse {
    Ok {
        result: EmailListResponse,
    },
    Labels {
        result: LabelListResponse,
    },
    LabelCreated {
        result: EmailLabel,
    },
    LabelDeleted {
        result: LabelDeleteResult,
    },
    LabelApplied {
        result: LabelApplyResult,
    },
    ReadEmail {
        result: EmailReadResponse,
    },
    MessageReadState {
        result: EmailReadState,
    },
    DeletionMarked {
        result: EmailDeletionMark,
    },
    EmailTrashed {
        result: EmailTrashResult,
    },
    Drafts {
        result: DraftListResponse,
    },
    DraftCreated {
        result: DraftCreateResult,
    },
    DraftDeletionMarked {
        result: DraftActionMark,
    },
    DraftSendingMarked {
        result: DraftActionMark,
    },
    DraftDeleted {
        result: DraftDeleteResult,
    },
    DraftSent {
        result: DraftSendResult,
    },
    Ready,
    Error {
        code: BrokerErrorCode,
        message: String,
    },
}

impl BrokerResponse {
    pub fn error(code: BrokerErrorCode, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/mcp.rs"]
mod tests;

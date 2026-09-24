use crate::gmail::{
    CreateLabelRequest, DeleteLabelRequest, EmailLabel, EmailListResponse, EmailReadResponse,
    EmailReadState, LabelDeleteResult, LabelListResponse, ListEmailsRequest, ReadEmailRequest,
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

    pub fn read_email(request: ReadEmailRequest) -> Self {
        Self::ReadEmail { request }
    }

    pub fn mark_email_read(request: ReadEmailRequest) -> Self {
        Self::MarkEmailRead { request }
    }

    pub fn mark_email_unread(request: ReadEmailRequest) -> Self {
        Self::MarkEmailUnread { request }
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
    Internal,
    InvalidLabelName,
    LabelAlreadyExists,
    InvalidLabelId,
    LabelNotFound,
    SystemLabel,
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
            Self::Internal => "internal",
            Self::InvalidLabelName => "invalid_label_name",
            Self::LabelAlreadyExists => "label_already_exists",
            Self::InvalidLabelId => "invalid_label_id",
            Self::LabelNotFound => "label_not_found",
            Self::SystemLabel => "system_label",
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
    ReadEmail {
        result: EmailReadResponse,
    },
    MessageReadState {
        result: EmailReadState,
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
mod tests {
    use super::{BrokerErrorCode, BrokerRequest, BrokerResponse};
    use crate::gmail::{
        CreateLabelRequest, DeleteLabelRequest, EmailLabel, EmailLabelType, EmailReadState,
        LabelDeleteResult, LabelListResponse, ListEmailsRequest, ReadEmailRequest,
    };

    #[test]
    fn broker_request_round_trips_the_list_contract() {
        let request = BrokerRequest::list_emails(ListEmailsRequest::default());
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: BrokerRequest = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded.validate().unwrap(),
            BrokerRequest::ListEmails { request } if request.effective_query() == "in:inbox"
        ));
    }

    #[test]
    fn broker_request_round_trips_list_labels_without_inputs() {
        let request = BrokerRequest::list_labels();
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(encoded, serde_json::json!({"operation": "list_labels"}));
        let decoded: BrokerRequest = serde_json::from_value(encoded).unwrap();
        assert!(matches!(
            decoded.validate().unwrap(),
            BrokerRequest::ListLabels
        ));
    }

    #[test]
    fn broker_label_response_preserves_label_ids_and_types() {
        let response = BrokerResponse::Labels {
            result: LabelListResponse {
                labels: vec![EmailLabel {
                    id: "Label_7".into(),
                    name: "Project Atlas".into(),
                    label_type: EmailLabelType::User,
                }],
            },
        };
        let encoded = serde_json::to_value(response).unwrap();
        assert_eq!(encoded["status"], "labels");
        assert_eq!(encoded["result"]["labels"][0]["id"], "Label_7");
        assert_eq!(encoded["result"]["labels"][0]["name"], "Project Atlas");
        assert_eq!(encoded["result"]["labels"][0]["type"], "user");
    }

    #[test]
    fn broker_create_and_delete_label_contracts_preserve_names_and_opaque_ids() {
        let create = BrokerRequest::create_label(CreateLabelRequest {
            name: "Project Atlas/2026".into(),
        });
        let encoded = serde_json::to_value(create).unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({"operation":"create_label","name":"Project Atlas/2026"})
        );
        assert!(matches!(
            serde_json::from_value::<BrokerRequest>(encoded).unwrap().validate().unwrap(),
            BrokerRequest::CreateLabel { request } if request.name == "Project Atlas/2026"
        ));

        let listed_id = "Label_7";
        let delete = BrokerRequest::delete_label(DeleteLabelRequest {
            label_id: listed_id.into(),
        });
        let encoded = serde_json::to_value(delete).unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({"operation":"delete_label","label_id":"Label_7"})
        );
        assert!(matches!(
            serde_json::from_value::<BrokerRequest>(encoded).unwrap().validate().unwrap(),
            BrokerRequest::DeleteLabel { request } if request.label_id == listed_id
        ));
        assert_eq!(
            serde_json::to_value(BrokerResponse::LabelCreated {
                result: EmailLabel {
                    id: listed_id.into(),
                    name: "Project Atlas".into(),
                    label_type: EmailLabelType::User,
                },
            })
            .unwrap()["result"],
            serde_json::json!({"id":"Label_7","name":"Project Atlas","type":"user"})
        );
        assert_eq!(
            serde_json::to_value(BrokerResponse::LabelDeleted {
                result: LabelDeleteResult {
                    label_id: listed_id.into(),
                    deleted: true,
                },
            })
            .unwrap()["result"],
            serde_json::json!({"label_id":"Label_7","deleted":true})
        );
    }

    #[test]
    fn broker_label_requests_reject_invalid_values_and_unknown_fields() {
        for request in [
            r#"{"operation":"create_label","name":"  "}"#,
            r#"{"operation":"create_label","name":"bad\nlabel"}"#,
            r#"{"operation":"create_label","name":"valid","account_id":"other"}"#,
        ] {
            assert!(match serde_json::from_str::<BrokerRequest>(request) {
                Ok(decoded) => decoded.validate().is_err(),
                Err(_) => true,
            });
        }
        for request in [
            r#"{"operation":"delete_label","label_id":""}"#,
            r#"{"operation":"delete_label","label_id":"Label_7","email":"other@example.com"}"#,
        ] {
            assert!(match serde_json::from_str::<BrokerRequest>(request) {
                Ok(decoded) => decoded.validate().is_err(),
                Err(_) => true,
            });
        }
    }

    #[test]
    fn broker_request_rejects_unknown_operations() {
        assert!(
            serde_json::from_str::<BrokerRequest>(
                r#"{"operation":"read_message","message_id":"abc123"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn broker_request_round_trips_read_email_without_an_account_identifier() {
        let request = BrokerRequest::read_email(ReadEmailRequest {
            message_id: "18abc_123-ef".into(),
        });
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(encoded["operation"], "read_email");
        assert_eq!(encoded["message_id"], "18abc_123-ef");
        assert!(encoded.get("account_id").is_none());
        assert!(encoded.get("email").is_none());
        let decoded: BrokerRequest = serde_json::from_value(encoded).unwrap();
        assert!(matches!(
            decoded.validate().unwrap(),
            BrokerRequest::ReadEmail { request }
                if request.message_id == "18abc_123-ef"
        ));
    }

    #[test]
    fn broker_read_state_operations_are_distinct_and_require_only_message_id() {
        for (request, operation) in [
            (
                BrokerRequest::mark_email_read(ReadEmailRequest {
                    message_id: "message-123".into(),
                }),
                "mark_email_read",
            ),
            (
                BrokerRequest::mark_email_unread(ReadEmailRequest {
                    message_id: "message-123".into(),
                }),
                "mark_email_unread",
            ),
        ] {
            let encoded = serde_json::to_value(&request).unwrap();
            assert_eq!(encoded["operation"], operation);
            assert_eq!(encoded["message_id"], "message-123");
            assert_eq!(encoded.as_object().unwrap().len(), 2);
            assert!(request.validate().is_ok());
        }
        for input in [
            r#"{"operation":"mark_email_read","message_id":"message-123","account_id":"other"}"#,
            r#"{"operation":"mark_email_unread","message_id":"message-123","thread_id":"thread-456"}"#,
        ] {
            assert!(serde_json::from_str::<BrokerRequest>(input).is_err());
        }
    }

    #[test]
    fn broker_message_read_state_response_is_small_and_typed() {
        let response = BrokerResponse::MessageReadState {
            result: EmailReadState {
                message_id: "message-123".into(),
                is_read: false,
            },
        };
        let encoded = serde_json::to_value(response).unwrap();
        assert_eq!(encoded["status"], "message_read_state");
        assert_eq!(
            encoded["result"],
            serde_json::json!({"message_id":"message-123","is_read":false})
        );
    }

    #[test]
    fn readiness_request_has_a_separate_internal_operation() {
        let request = BrokerRequest::readiness();
        let encoded = serde_json::to_string(&request).unwrap();
        assert!(encoded.contains(r#""operation":"readiness""#));
        let decoded: BrokerRequest = serde_json::from_str(&encoded).unwrap();
        assert!(decoded.validate_readiness().is_ok());
    }

    #[test]
    fn broker_error_serialization_does_not_include_credentials() {
        let response = BrokerResponse::error(
            BrokerErrorCode::ReauthenticationRequired,
            "reauthenticate the selected target",
        );
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("reauthentication_required"));
        assert!(!encoded.contains("refresh_token"));
        assert!(!encoded.contains("access_token"));
    }

    #[test]
    fn credential_unavailable_uses_a_stable_public_code() {
        let response = BrokerResponse::error(
            BrokerErrorCode::CredentialUnavailable,
            "the selected account credential is temporarily unavailable",
        );
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("credential_unavailable"));
        assert!(!encoded.contains("refresh_token"));
        assert!(!encoded.contains("access_token"));
    }
}

use crate::gmail::{EmailListResponse, ListEmailsRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerRequest {
    pub operation: String,
    #[serde(flatten)]
    pub request: ListEmailsRequest,
}

impl BrokerRequest {
    pub fn list_emails(request: ListEmailsRequest) -> Self {
        Self {
            operation: "list_emails".into(),
            request,
        }
    }

    pub fn readiness() -> Self {
        Self {
            operation: "readiness".into(),
            request: ListEmailsRequest::default(),
        }
    }

    pub fn validate(self) -> anyhow::Result<ListEmailsRequest> {
        anyhow::ensure!(
            self.operation == "list_emails",
            "unsupported broker operation"
        );
        self.request.validate()
    }

    pub fn validate_readiness(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.operation == "readiness",
            "unsupported broker operation"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerErrorCode {
    InvalidRequest,
    TargetNotConfigured,
    TargetUnavailable,
    ReauthenticationRequired,
    CredentialUnavailable,
    GmailRateLimited,
    GmailUnavailable,
    Internal,
}

impl BrokerErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::TargetNotConfigured => "target_not_configured",
            Self::TargetUnavailable => "target_unavailable",
            Self::ReauthenticationRequired => "reauthentication_required",
            Self::CredentialUnavailable => "credential_unavailable",
            Self::GmailRateLimited => "gmail_rate_limited",
            Self::GmailUnavailable => "gmail_unavailable",
            Self::Internal => "internal",
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
    use crate::gmail::ListEmailsRequest;

    #[test]
    fn broker_request_round_trips_the_list_contract() {
        let request = BrokerRequest::list_emails(ListEmailsRequest::default());
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: BrokerRequest = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.validate().unwrap().effective_query(), "in:inbox");
    }

    #[test]
    fn broker_request_rejects_unknown_operations() {
        let request: BrokerRequest =
            serde_json::from_str(r#"{"operation":"read_message","max_results":1}"#).unwrap();
        assert!(request.validate().is_err());
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

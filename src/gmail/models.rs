// SPDX-License-Identifier: MPL-2.0
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailSummary {
    pub id: String,
    pub thread_id: String,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub labels: Vec<String>,
    pub snippet: String,
    pub snippet_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailListResponse {
    pub target_email: String,
    pub messages: Vec<EmailSummary>,
    pub next_page_token: Option<String>,
    pub result_size_estimate: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmailLabelType {
    System,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailLabel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: EmailLabelType,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabelListResponse {
    #[serde(default)]
    pub labels: Vec<EmailLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmailBodyStatus {
    Complete,
    NoReadableBody,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailRecipients {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailReadResponse {
    pub message_id: String,
    pub thread_id: String,
    pub from: Option<String>,
    pub recipients: EmailRecipients,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub labels: Vec<String>,
    pub body_text: Option<String>,
    pub body_status: EmailBodyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailReadState {
    pub message_id: String,
    pub is_read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabelDeleteResult {
    pub label_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LabelApplyResult {
    pub message_id: String,
    pub label_id: String,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DeleteMarkedEmailRequest {
    /// One-use marker returned by `mark_email_for_deletion`.
    #[schemars(length(min = 32, max = 32), regex(pattern = "^[0-9a-f]{32}$"))]
    pub marker_id: String,
}

impl DeleteMarkedEmailRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            self.marker_id.len() == 32
                && self
                    .marker_id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "marker_id must be a 32-character deletion marker returned by mark_email_for_deletion"
        );
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailDeletionMark {
    pub marker_id: String,
    pub message_id: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct EmailTrashResult {
    pub message_id: String,
    pub trashed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateDraftRequest {
    /// One recipient address. Header control characters are rejected.
    #[schemars(length(min = 3, max = 320))]
    pub to: String,
    #[schemars(length(min = 1, max = 998))]
    pub subject: String,
    #[schemars(length(min = 1, max = 24_576))]
    pub body: String,
}

impl CreateDraftRequest {
    pub fn validate(self) -> Result<Self> {
        validate_draft_header("to", &self.to, 320)?;
        let (local, domain) = self
            .to
            .split_once('@')
            .ok_or_else(|| anyhow::anyhow!("to must be an email address"))?;
        anyhow::ensure!(
            !local.is_empty() && !domain.is_empty() && self.to.matches('@').count() == 1,
            "to must contain one valid email address"
        );
        anyhow::ensure!(
            !self.to.chars().any(char::is_whitespace),
            "to must be a plain email address without whitespace"
        );
        validate_draft_header("subject", &self.subject, 998)?;
        anyhow::ensure!(!self.subject.trim().is_empty(), "subject cannot be blank");
        validate_draft_body(&self.body)?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateReplyDraftRequest {
    pub message_id: String,
    #[schemars(length(min = 1, max = 24_576))]
    pub body: String,
}

impl CreateReplyDraftRequest {
    pub fn validate(self) -> Result<Self> {
        ReadEmailRequest {
            message_id: self.message_id.clone(),
        }
        .validate()?;
        validate_draft_body(&self.body)?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ListDraftsRequest {
    #[serde(default = "default_max_results")]
    #[schemars(range(min = 1, max = 50), default = "default_max_results")]
    pub max_results: u32,
    #[serde(default)]
    #[schemars(
        length(min = 1, max = 4096),
        regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]*$")
    )]
    pub page_token: Option<String>,
}

impl Default for ListDraftsRequest {
    fn default() -> Self {
        Self {
            max_results: DEFAULT_MAX_RESULTS,
            page_token: None,
        }
    }
}

impl ListDraftsRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            (1..=MAX_MAX_RESULTS).contains(&self.max_results),
            "max_results must be between 1 and 50"
        );
        if let Some(token) = &self.page_token {
            anyhow::ensure!(
                !token.is_empty() && token.len() <= 4096 && !token.chars().any(char::is_control),
                "page_token is invalid"
            );
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftSummary {
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: String,
    pub to: Vec<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftListResponse {
    pub target_email: String,
    pub drafts: Vec<DraftSummary>,
    pub next_page_token: Option<String>,
    pub result_size_estimate: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftCreateResult {
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DraftIdRequest {
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[A-Za-z0-9_-]+$"))]
    pub draft_id: String,
}

impl DraftIdRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            (1..=256).contains(&self.draft_id.len())
                && self
                    .draft_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "draft_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
        );
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ActionMarkerRequest {
    #[schemars(length(min = 32, max = 32), regex(pattern = "^[0-9a-f]{32}$"))]
    pub marker_id: String,
}

impl ActionMarkerRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            self.marker_id.len() == 32
                && self
                    .marker_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "marker_id must be a 32-character action marker"
        );
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftActionMark {
    pub marker_id: String,
    pub draft_id: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftDeleteResult {
    pub draft_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct DraftSendResult {
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: String,
}

fn validate_draft_header(name: &str, value: &str, max_bytes: usize) -> Result<()> {
    anyhow::ensure!(!value.trim().is_empty(), "{name} cannot be blank");
    anyhow::ensure!(value.len() <= max_bytes, "{name} exceeds {max_bytes} bytes");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains control characters"
    );
    Ok(())
}

fn validate_draft_body(body: &str) -> Result<()> {
    anyhow::ensure!(!body.trim().is_empty(), "body cannot be blank");
    anyhow::ensure!(body.len() <= 24_576, "body exceeds the 24 KiB draft limit");
    anyhow::ensure!(
        !body
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')),
        "body contains unsupported control characters"
    );
    Ok(())
}

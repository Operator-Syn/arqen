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

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

use anyhow::{Context, Result};
use reqwest::{StatusCode, blocking::Client};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};
use url::Url;

pub const GMAIL_API_BASE_URL: &str = "https://gmail.googleapis.com/gmail/v1/";
pub const DEFAULT_MAX_RESULTS: u32 = 20;
pub const MAX_MAX_RESULTS: u32 = 50;
const MAX_QUERY_LENGTH: usize = 1_024;
const MAX_PAGE_TOKEN_LENGTH: usize = 4_096;
const MAX_LABEL_IDS: usize = 20;
const MAX_LABEL_ID_LENGTH: usize = 256;
const MAX_SNIPPET_CHARS: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ListEmailsRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default = "default_max_results")]
    pub max_results: u32,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub include_spam_trash: bool,
}

fn default_max_results() -> u32 {
    DEFAULT_MAX_RESULTS
}

impl Default for ListEmailsRequest {
    fn default() -> Self {
        Self {
            query: None,
            label_ids: Vec::new(),
            max_results: DEFAULT_MAX_RESULTS,
            page_token: None,
            include_spam_trash: false,
        }
    }
}

impl ListEmailsRequest {
    pub fn validate(mut self) -> Result<Self> {
        if let Some(query) = self.query.take() {
            validate_text("query", &query, MAX_QUERY_LENGTH)?;
            let query = query.trim().to_owned();
            self.query = (!query.is_empty()).then_some(query);
        }
        anyhow::ensure!(
            (1..=MAX_MAX_RESULTS).contains(&self.max_results),
            "max_results must be between 1 and {MAX_MAX_RESULTS}"
        );
        anyhow::ensure!(
            self.label_ids.len() <= MAX_LABEL_IDS,
            "at most {MAX_LABEL_IDS} label IDs may be requested"
        );
        for label_id in &self.label_ids {
            anyhow::ensure!(!label_id.is_empty(), "label IDs cannot be empty");
            validate_text("label ID", label_id, MAX_LABEL_ID_LENGTH)?;
        }
        if let Some(page_token) = &self.page_token {
            anyhow::ensure!(!page_token.is_empty(), "page_token cannot be empty");
            validate_text("page token", page_token, MAX_PAGE_TOKEN_LENGTH)?;
        }
        Ok(self)
    }

    pub fn effective_query(&self) -> &str {
        self.query.as_deref().unwrap_or("in:inbox")
    }
}

fn validate_text(name: &str, value: &str, max_length: usize) -> Result<()> {
    anyhow::ensure!(
        value.chars().count() <= max_length,
        "{name} exceeds the maximum length of {max_length} characters"
    );
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}

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

#[derive(Debug)]
pub struct GmailApiError {
    status: StatusCode,
    message: String,
}

impl GmailApiError {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[cfg(test)]
    pub(crate) fn for_test(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl fmt::Display for GmailApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Gmail API returned HTTP {}: {}",
            self.status, self.message
        )
    }
}

impl std::error::Error for GmailApiError {}

pub fn is_unauthorized(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<GmailApiError>()
        .is_some_and(|error| error.status == StatusCode::UNAUTHORIZED)
}

#[derive(Debug, Clone, Deserialize)]
struct MessageListResponse {
    #[serde(default)]
    messages: Vec<MessageReference>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    result_size_estimate: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageReference {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageResource {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
    #[serde(rename = "labelIds", default)]
    label_ids: Vec<String>,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    payload: Option<MessagePayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessagePayload {
    #[serde(default)]
    headers: Vec<MessageHeader>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageHeader {
    name: String,
    value: String,
}

pub struct GmailApi {
    client: Client,
    base_url: Url,
}

impl GmailApi {
    pub fn new() -> Result<Self> {
        Self::with_base_url(GMAIL_API_BASE_URL)
    }

    pub fn with_base_url(base_url: &str) -> Result<Self> {
        let base_url = Url::parse(base_url).context("parse Gmail API base URL")?;
        anyhow::ensure!(
            base_url.scheme() == "http" || base_url.scheme() == "https",
            "Gmail API base URL must use HTTP or HTTPS"
        );
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("create Gmail API client")?;
        Ok(Self { client, base_url })
    }

    #[cfg(test)]
    pub fn with_client(base_url: &str, client: Client) -> Result<Self> {
        let base_url = Url::parse(base_url).context("parse Gmail API base URL")?;
        Ok(Self { client, base_url })
    }

    pub fn list_emails(
        &self,
        access_token: &str,
        target_email: &str,
        request: ListEmailsRequest,
    ) -> Result<EmailListResponse> {
        let request = request.validate()?;
        let list_url = self.base_url.join("users/me/messages")?;
        let mut query = vec![
            ("q", request.effective_query().to_owned()),
            ("maxResults", request.max_results.to_string()),
            ("includeSpamTrash", request.include_spam_trash.to_string()),
            (
                "fields",
                "messages(id,threadId),nextPageToken,resultSizeEstimate".into(),
            ),
        ];
        if let Some(page_token) = request.page_token {
            query.push(("pageToken", page_token));
        }
        for label_id in request.label_ids {
            query.push(("labelIds", label_id));
        }
        let listed: MessageListResponse = self
            .client
            .get(list_url)
            .bearer_auth(access_token)
            .query(&query)
            .send()
            .context("request Gmail message list")
            .and_then(parse_json_response)?;

        let mut messages = Vec::with_capacity(listed.messages.len());
        for reference in listed.messages {
            let mut message_url = self.base_url.join("users/me/messages")?;
            message_url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
                .push(&reference.id);
            let detail: MessageResource = self
                .client
                .get(message_url)
                .bearer_auth(access_token)
                .query(&[
                    ("format", "metadata"),
                    ("metadataHeaders", "From"),
                    ("metadataHeaders", "Subject"),
                    ("metadataHeaders", "Date"),
                    (
                        "fields",
                        "id,threadId,labelIds,snippet,payload(headers(name,value))",
                    ),
                ])
                .send()
                .with_context(|| format!("request Gmail message metadata for {}", reference.id))
                .and_then(parse_json_response)?;
            messages.push(email_summary(detail, &reference));
        }

        Ok(EmailListResponse {
            target_email: target_email.to_owned(),
            messages,
            next_page_token: listed.next_page_token,
            result_size_estimate: listed.result_size_estimate,
        })
    }
}

fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let message = response
            .json::<GoogleErrorResponse>()
            .ok()
            .and_then(|error| error.error)
            .and_then(|error| error.message)
            .unwrap_or_else(|| "the upstream request failed".into());
        return Err(anyhow::Error::new(GmailApiError { status, message }));
    }
    response.json().context("parse Gmail API response")
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    error: Option<GoogleErrorBody>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorBody {
    message: Option<String>,
}

fn email_summary(detail: MessageResource, reference: &MessageReference) -> EmailSummary {
    let headers = detail
        .payload
        .map(|payload| payload.headers)
        .unwrap_or_default();
    let header = |name: &str| {
        headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.clone())
    };
    let (snippet, snippet_truncated) = truncate_snippet(&detail.snippet);
    EmailSummary {
        id: if detail.id.is_empty() {
            reference.id.clone()
        } else {
            detail.id
        },
        thread_id: if detail.thread_id.is_empty() {
            reference.thread_id.clone()
        } else {
            detail.thread_id
        },
        from: header("From"),
        subject: header("Subject"),
        date: header("Date"),
        labels: detail.label_ids,
        snippet,
        snippet_truncated,
    }
}

fn truncate_snippet(snippet: &str) -> (String, bool) {
    let mut chars = snippet.chars();
    let truncated: String = chars.by_ref().take(MAX_SNIPPET_CHARS).collect();
    (truncated, chars.next().is_some())
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MAX_RESULTS, GmailApi, ListEmailsRequest, MAX_MAX_RESULTS, truncate_snippet,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn request_defaults_to_inbox_and_bounded_page_size() {
        let request = ListEmailsRequest::default();
        assert_eq!(request.effective_query(), "in:inbox");
        assert_eq!(request.max_results, DEFAULT_MAX_RESULTS);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn request_rejects_invalid_limits_and_control_characters() {
        let mut request = ListEmailsRequest {
            max_results: MAX_MAX_RESULTS + 1,
            ..Default::default()
        };
        assert!(request.clone().validate().is_err());
        request.max_results = 1;
        request.query = Some("from:test\n".into());
        assert!(request.validate().is_err());
    }

    #[test]
    fn snippet_truncation_is_unicode_safe_and_reports_overflow() {
        let snippet = "界".repeat(301);
        let (truncated, was_truncated) = truncate_snippet(&snippet);
        assert_eq!(truncated.chars().count(), 300);
        assert!(was_truncated);
    }

    #[test]
    fn lists_messages_with_bounded_metadata_requests() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let server = thread::spawn(move || {
            let responses = [
                r#"{"messages":[{"id":"message-1","threadId":"thread-1"}],"nextPageToken":"next","resultSizeEstimate":1}"#,
                r#"{"id":"message-1","threadId":"thread-1","labelIds":["INBOX"],"snippet":"A short message","payload":{"headers":[{"name":"subject","value":"Hello"},{"name":"FROM","value":"sender@example.com"},{"name":"Date","value":"Mon, 1 Jan 2024 00:00:00 +0000"}]}}"#,
            ];
            for body in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(request).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });

        let api = GmailApi::with_base_url(&format!("http://{address}/")).unwrap();
        let result = api
            .list_emails(
                "test-access-token",
                "target@example.com",
                ListEmailsRequest {
                    query: Some(" from:sender@example.com ".into()),
                    label_ids: vec!["INBOX".into(), "IMPORTANT".into()],
                    max_results: 5,
                    page_token: Some("page-1".into()),
                    include_spam_trash: true,
                },
            )
            .unwrap();
        server.join().unwrap();

        assert_eq!(result.target_email, "target@example.com");
        assert_eq!(result.next_page_token.as_deref(), Some("next"));
        assert_eq!(result.messages[0].subject.as_deref(), Some("Hello"));
        assert_eq!(
            result.messages[0].from.as_deref(),
            Some("sender@example.com")
        );
        assert_eq!(result.messages[0].labels, vec!["INBOX".to_owned()]);

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /users/me/messages?"));
        assert!(requests[0].contains("q=from%3Asender%40example.com"));
        assert!(requests[0].contains("maxResults=5"));
        assert!(requests[0].contains("includeSpamTrash=true"));
        assert!(requests[0].contains("pageToken=page-1"));
        assert!(requests[0].contains("labelIds=INBOX"));
        assert!(requests[0].contains("labelIds=IMPORTANT"));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer test-access-token")
        );
        assert!(
            requests[1].starts_with("GET /users/me/messages/message-1?"),
            "captured requests: {requests:?}"
        );
        assert!(requests[1].contains("format=metadata"));
        assert!(requests[1].contains("metadataHeaders=From"));
        assert!(requests[1].contains("metadataHeaders=Subject"));
        assert!(requests[1].contains("metadataHeaders=Date"));
    }

    #[test]
    fn maps_upstream_failures_without_echoing_credentials() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"{"error":{"message":"token rejected"}}"#;
            write!(
                stream,
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let api = GmailApi::with_base_url(&format!("http://{address}/")).unwrap();
        let error = api
            .list_emails(
                "secret-access-token",
                "target@example.com",
                Default::default(),
            )
            .unwrap_err();
        server.join().unwrap();
        assert!(super::is_unauthorized(&error));
        assert!(error.to_string().contains("token rejected"));
        assert!(!error.to_string().contains("secret-access-token"));
    }
}

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

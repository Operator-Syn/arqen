// SPDX-License-Identifier: MPL-2.0
use super::*;

#[path = "client/drafts.rs"]
pub(super) mod drafts;

#[path = "responses.rs"]
mod responses;
use responses::*;
#[cfg(test)]
pub(super) use responses::{email_read_response, read_body_text, truncate_snippet};

#[derive(Debug)]
pub struct GmailApiError {
    status: StatusCode,
}

impl GmailApiError {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[cfg(test)]
    pub(crate) fn for_test(status: StatusCode, _message: impl Into<String>) -> Self {
        Self { status }
    }
}

impl fmt::Display for GmailApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Gmail API returned HTTP {}", self.status)
    }
}

impl std::error::Error for GmailApiError {}

// Do not retain a provider error as a source: a follow-up GET 401 must not
// trigger the broker's write retry, and transport errors can contain URLs.
#[derive(Debug)]
struct LabelWriteUncertain {
    stage: &'static str,
    category: &'static str,
    status: Option<u16>,
}

impl fmt::Display for LabelWriteUncertain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the label write may have succeeded; verify with list_labels before retrying (stage={}, category={}, status={})",
            self.stage,
            self.category,
            self.status
                .map_or_else(|| "unknown".to_owned(), |status| status.to_string()),
        )
    }
}
impl std::error::Error for LabelWriteUncertain {}

fn uncertain_label_write(
    stage: &'static str,
    category: &'static str,
    status: Option<u16>,
) -> anyhow::Error {
    anyhow::Error::new(LabelWriteUncertain {
        stage,
        category,
        status,
    })
}

#[derive(Debug)]
pub(crate) struct ReadEmailTooLarge;

impl fmt::Display for ReadEmailTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail message exceeds the configured read limit")
    }
}

impl std::error::Error for ReadEmailTooLarge {}

#[derive(Debug)]
pub(crate) struct SystemLabelError;

impl fmt::Display for SystemLabelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail system labels cannot be deleted")
    }
}

impl std::error::Error for SystemLabelError {}

#[derive(Debug)]
pub(crate) struct DraftMessageMutationError;

impl fmt::Display for DraftMessageMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail draft messages only support draft-specific operations")
    }
}
impl std::error::Error for DraftMessageMutationError {}

#[derive(Debug)]
pub(crate) enum ApplyLabelFailure {
    InvalidMessageId,
    InvalidLabelId,
    LabelLookup(anyhow::Error),
    SystemLabel,
    MessageModify(anyhow::Error),
}

impl ApplyLabelFailure {
    pub(crate) fn is_unauthorized(&self) -> bool {
        match self {
            Self::LabelLookup(error) | Self::MessageModify(error) => is_unauthorized(error),
            Self::InvalidMessageId | Self::InvalidLabelId | Self::SystemLabel => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct LabelNotFoundError;

impl fmt::Display for LabelNotFoundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail label was not found")
    }
}

impl std::error::Error for LabelNotFoundError {}

pub(crate) fn is_read_email_too_large(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ReadEmailTooLarge>().is_some()
}

pub fn is_unauthorized(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<GmailApiError>()
        .is_some_and(|error| error.status == StatusCode::UNAUTHORIZED)
}

pub(crate) fn is_draft_message_mutation(error: &anyhow::Error) -> bool {
    error.downcast_ref::<DraftMessageMutationError>().is_some()
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
pub(super) struct MessageResource {
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

#[derive(Debug, Deserialize)]
struct ModifiedMessageResource {
    id: String,
    #[serde(rename = "labelIds")]
    label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TrashedMessageResource {
    id: String,
}

#[derive(Debug, Deserialize)]
struct MessageLabelsResource {
    id: String,
    #[serde(rename = "labelIds", default)]
    label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LabelTypeResponse {
    id: String,
    #[serde(rename = "type")]
    label_type: crate::gmail::EmailLabelType,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct MessagePayload {
    #[serde(default, rename = "mimeType")]
    mime_type: String,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    headers: Vec<MessageHeader>,
    #[serde(default)]
    body: Option<MessagePartBody>,
    #[serde(default)]
    parts: Vec<MessagePayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessagePartBody {
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    size: usize,
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
    pub(crate) fn is_label_write_uncertain(error: &anyhow::Error) -> bool {
        error.downcast_ref::<LabelWriteUncertain>().is_some()
    }

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

    pub fn list_labels(&self, access_token: &str) -> Result<crate::gmail::LabelListResponse> {
        let labels_url = self.base_url.join("users/me/labels")?;
        self.client
            .get(labels_url)
            .bearer_auth(access_token)
            .query(&[("fields", "labels(id,name,type)")])
            .send()
            .context("request Gmail labels")
            .and_then(parse_json_response)
    }

    pub fn create_label(
        &self,
        access_token: &str,
        request: crate::gmail::CreateLabelRequest,
    ) -> Result<crate::gmail::EmailLabel> {
        let request = request.validate()?;
        let labels_url = self.base_url.join("users/me/labels")?;
        #[derive(Deserialize)]
        struct CreatedLabel {
            id: String,
            name: String,
            #[serde(rename = "type", default, deserialize_with = "present_label_type")]
            label_type: Option<crate::gmail::EmailLabelType>,
        }
        // Only an absent field allows a lookup; explicit null/unknown types fail closed.
        fn present_label_type<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> std::result::Result<Option<crate::gmail::EmailLabelType>, D::Error> {
            crate::gmail::EmailLabelType::deserialize(deserializer).map(Some)
        }
        let response = self
            .client
            .post(labels_url)
            .bearer_auth(access_token)
            .query(&[("fields", "id,name,type")])
            .json(&serde_json::json!({"name": request.name}))
            .send()
            .map_err(|_| uncertain_label_write("create_request", "transport", None))?;
        let status = response.status().as_u16();
        let successful_write = response.status().is_success();
        let label: CreatedLabel = parse_json_response(response).map_err(|error| {
            if successful_write {
                uncertain_label_write("create_response", "response", Some(status))
            } else {
                error
            }
        })?;
        if label.id.is_empty()
            || label.id.chars().any(char::is_control)
            || label.name != request.name
        {
            return Err(uncertain_label_write(
                "create_identity",
                "validation",
                Some(status),
            ));
        }
        let expected_id = label.id.clone();
        let label = match label.label_type {
            Some(label_type) => crate::gmail::EmailLabel {
                id: label.id,
                name: label.name,
                label_type,
            },
            None => {
                let mut url = self
                    .base_url
                    .join("users/me/labels")
                    .map_err(|_| uncertain_label_write("verify_request", "configuration", None))?;
                url.path_segments_mut()
                    .map_err(|_| uncertain_label_write("verify_request", "configuration", None))?
                    .push(&label.id);
                let response = self
                    .client
                    .get(url)
                    .bearer_auth(access_token)
                    .query(&[("fields", "id,name,type")])
                    .send()
                    .map_err(|_| uncertain_label_write("verify_request", "transport", None))?;
                let verification_status = response.status();
                parse_json_response(response).map_err(|_| {
                    uncertain_label_write(
                        "verify_response",
                        if verification_status.is_success() {
                            "response"
                        } else {
                            "provider"
                        },
                        Some(verification_status.as_u16()),
                    )
                })?
            }
        };
        if label.id != expected_id
            || label.name != request.name
            || label.label_type != crate::gmail::EmailLabelType::User
        {
            return Err(uncertain_label_write("verify_identity", "validation", None));
        }
        Ok(label)
    }

    pub fn delete_label(
        &self,
        access_token: &str,
        request: crate::gmail::DeleteLabelRequest,
    ) -> Result<crate::gmail::LabelDeleteResult> {
        let request = request.validate()?;
        let mut label_url = self.base_url.join("users/me/labels")?;
        label_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(&request.label_id);

        // Gmail's system-label set is not exhaustive in its documentation. Fetch
        // only the owner type so unknown reserved/system IDs fail closed too.
        let label: LabelTypeResponse = self
            .client
            .get(label_url.clone())
            .bearer_auth(access_token)
            .query(&[("fields", "id,type")])
            .send()
            .context("check Gmail label type before deletion")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            label.id == request.label_id,
            "Gmail returned an unexpected label ID"
        );
        if label.label_type == crate::gmail::EmailLabelType::System {
            return Err(anyhow::Error::new(SystemLabelError));
        }
        anyhow::ensure!(
            label.label_type == crate::gmail::EmailLabelType::User,
            "Gmail returned an unknown label type"
        );

        let response = self
            .client
            .delete(label_url)
            .bearer_auth(access_token)
            .send()
            .map_err(|_| uncertain_label_write("delete_request", "transport", None))?;
        let status = response.status().as_u16();
        let successful_write = response.status().is_success();
        parse_empty_json_response(response).map_err(|error| {
            if successful_write {
                uncertain_label_write("delete_response", "response", Some(status))
            } else {
                error
            }
        })?;
        Ok(crate::gmail::LabelDeleteResult {
            label_id: request.label_id,
            deleted: true,
        })
    }

    pub(crate) fn apply_label(
        &self,
        access_token: &str,
        request: crate::gmail::ApplyLabelRequest,
    ) -> std::result::Result<crate::gmail::LabelApplyResult, ApplyLabelFailure> {
        request
            .validate_message_id()
            .map_err(|_| ApplyLabelFailure::InvalidMessageId)?;
        request
            .validate_label_id()
            .map_err(|_| ApplyLabelFailure::InvalidLabelId)?;
        self.ensure_not_draft(access_token, &request.message_id)
            .map_err(ApplyLabelFailure::MessageModify)?;

        let mut label_url = self
            .base_url
            .join("users/me/labels")
            .map_err(|error| ApplyLabelFailure::LabelLookup(error.into()))?;
        label_url
            .path_segments_mut()
            .map_err(|_| {
                ApplyLabelFailure::LabelLookup(anyhow::anyhow!(
                    "Gmail API base URL cannot accept path segments"
                ))
            })?
            .push(&request.label_id);
        let label: LabelTypeResponse = self
            .client
            .get(label_url)
            .bearer_auth(access_token)
            .query(&[("fields", "id,type")])
            .send()
            .context("check Gmail label type before applying it")
            .and_then(parse_json_response)
            .map_err(|error| {
                if error
                    .downcast_ref::<GmailApiError>()
                    .is_some_and(|api_error| api_error.status() == StatusCode::NOT_FOUND)
                {
                    ApplyLabelFailure::LabelLookup(anyhow::Error::new(LabelNotFoundError))
                } else {
                    ApplyLabelFailure::LabelLookup(error)
                }
            })?;
        if label.id != request.label_id {
            return Err(ApplyLabelFailure::LabelLookup(anyhow::anyhow!(
                "Gmail returned an unexpected label ID"
            )));
        }
        if label.label_type == crate::gmail::EmailLabelType::System {
            return Err(ApplyLabelFailure::SystemLabel);
        }
        if label.label_type != crate::gmail::EmailLabelType::User {
            return Err(ApplyLabelFailure::LabelLookup(anyhow::anyhow!(
                "Gmail returned an unknown label type"
            )));
        }

        let mut message_url = self
            .base_url
            .join("users/me/messages")
            .map_err(|error| ApplyLabelFailure::MessageModify(error.into()))?;
        message_url
            .path_segments_mut()
            .map_err(|_| {
                ApplyLabelFailure::MessageModify(anyhow::anyhow!(
                    "Gmail API base URL cannot accept path segments"
                ))
            })?
            .push(&request.message_id)
            .push("modify");
        let message: ModifiedMessageResource = self
            .client
            .post(message_url)
            .bearer_auth(access_token)
            .query(&[("fields", "id,labelIds")])
            .json(&serde_json::json!({"addLabelIds": [request.label_id.clone()]}))
            .send()
            .context("apply Gmail label to message")
            .and_then(parse_json_response)
            .map_err(ApplyLabelFailure::MessageModify)?;
        if message.id != request.message_id {
            return Err(ApplyLabelFailure::MessageModify(anyhow::anyhow!(
                "Gmail returned an unexpected message ID"
            )));
        }
        if !message
            .label_ids
            .iter()
            .any(|label_id| label_id == &request.label_id)
        {
            return Err(ApplyLabelFailure::MessageModify(anyhow::anyhow!(
                "Gmail did not return the applied label"
            )));
        }

        Ok(crate::gmail::LabelApplyResult {
            message_id: request.message_id,
            label_id: request.label_id,
            applied: true,
        })
    }

    pub fn read_email(
        &self,
        access_token: &str,
        request: crate::gmail::ReadEmailRequest,
    ) -> Result<crate::gmail::EmailReadResponse> {
        let request = request.validate()?;
        let mut message_url = self.base_url.join("users/me/messages")?;
        message_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(&request.message_id);
        let message: MessageResource = self
            .client
            .get(message_url)
            .bearer_auth(access_token)
            .query(&[
                ("format", "full"),
                ("fields", "id,threadId,labelIds,payload"),
            ])
            .send()
            .context("request Gmail message")
            .and_then(parse_read_email_response)?;
        email_read_response(message)
    }

    pub fn mark_email_read(
        &self,
        access_token: &str,
        request: crate::gmail::ReadEmailRequest,
    ) -> Result<crate::gmail::EmailReadState> {
        self.modify_unread_label(access_token, request, true)
    }

    pub fn mark_email_unread(
        &self,
        access_token: &str,
        request: crate::gmail::ReadEmailRequest,
    ) -> Result<crate::gmail::EmailReadState> {
        self.modify_unread_label(access_token, request, false)
    }

    pub fn trash_email(
        &self,
        access_token: &str,
        request: crate::gmail::ReadEmailRequest,
    ) -> Result<crate::gmail::EmailTrashResult> {
        let request = request.validate()?;
        self.ensure_not_draft(access_token, &request.message_id)?;
        let mut message_url = self.base_url.join("users/me/messages")?;
        message_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(&request.message_id)
            .push("trash");
        let message: TrashedMessageResource = self
            .client
            .post(message_url)
            .bearer_auth(access_token)
            .query(&[("fields", "id")])
            // Gmail requires Content-Length even for an empty HTTP/1.1 POST.
            .body("")
            .send()
            .context("move Gmail message to Trash")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            message.id == request.message_id,
            "Gmail trash response has an unexpected message ID"
        );
        Ok(crate::gmail::EmailTrashResult {
            message_id: message.id,
            trashed: true,
        })
    }

    fn modify_unread_label(
        &self,
        access_token: &str,
        request: crate::gmail::ReadEmailRequest,
        is_read: bool,
    ) -> Result<crate::gmail::EmailReadState> {
        let request = request.validate()?;
        self.ensure_not_draft(access_token, &request.message_id)?;
        let mut message_url = self.base_url.join("users/me/messages")?;
        message_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(&request.message_id)
            .push("modify");
        let label_change = if is_read {
            serde_json::json!({"removeLabelIds": ["UNREAD"]})
        } else {
            serde_json::json!({"addLabelIds": ["UNREAD"]})
        };
        let message: ModifiedMessageResource = self
            .client
            .post(message_url)
            .bearer_auth(access_token)
            .query(&[("fields", "id,labelIds")])
            .json(&label_change)
            .send()
            .context("modify Gmail message unread label")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            !message.id.is_empty() && message.id == request.message_id,
            "Gmail modify response has an unexpected message ID"
        );
        Ok(crate::gmail::EmailReadState {
            message_id: message.id,
            is_read: !message.label_ids.iter().any(|label| label == "UNREAD"),
        })
    }

    fn ensure_not_draft(&self, access_token: &str, message_id: &str) -> Result<()> {
        let mut url = self.base_url.join("users/me/messages")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(message_id);
        let message: MessageLabelsResource = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .query(&[("format", "minimal"), ("fields", "id,labelIds")])
            .send()
            .context("check whether Gmail message is a draft")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            message.id == message_id,
            "Gmail returned an unexpected message ID"
        );
        if message.label_ids.iter().any(|label| label == "DRAFT") {
            return Err(anyhow::Error::new(DraftMessageMutationError));
        }
        Ok(())
    }
}

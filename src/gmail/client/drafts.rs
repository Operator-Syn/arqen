// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::gmail::{
    CreateDraftRequest, CreateReplyDraftRequest, DraftCreateResult, DraftListResponse,
    DraftSummary, ListDraftsRequest,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraftListStage {
    List,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraftListCategory {
    Transport,
    ProviderStatus,
    Decoding,
    Validation,
    ResponseTooLarge,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraftListReason {
    Transport,
    ProviderStatus,
    MissingRequiredField,
    InvalidJsonShape,
    InvalidJsonSyntax,
    ResponseTooLarge,
    Configuration,
    InvalidReferenceId,
    ReferenceDetailMismatch,
    EmptyDraftId,
    EmptyMessageId,
    EmptyThreadId,
    MissingDraftLabel,
    UnsupportedDraftLabels,
}

impl fmt::Display for DraftListReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail returned invalid draft metadata")
    }
}

impl std::error::Error for DraftListReason {}

// Only bounded, non-sensitive facts survive this boundary. In particular,
// never keep reqwest/serde errors, URLs, IDs, tokens or response bodies.
#[derive(Debug)]
pub(crate) struct DraftListError {
    pub(crate) stage: DraftListStage,
    pub(crate) category: DraftListCategory,
    pub(crate) status: Option<StatusCode>,
    pub(crate) reason: DraftListReason,
    // TEMPORARY: remove with the draft-label investigation (see broker docs).
    temporary_labels: Option<TemporaryDraftLabels>,
}

// Fixed vocabulary only; never retain provider strings in diagnostic facts.
#[derive(Debug)]
struct TemporaryDraftLabels {
    draft_present: bool,
    total: usize,
    system: Vec<&'static str>,
    custom: usize,
    unknown: usize,
}

impl TemporaryDraftLabels {
    fn from_labels(labels: &[String]) -> Self {
        // Sorted fixed allowlist, not a provider-derived vocabulary. Unknown
        // future system values remain anonymous. Recognition is not validation.
        const SYSTEM: &[&str] = &[
            "CATEGORY_FORUMS",
            "CATEGORY_PERSONAL",
            "CATEGORY_PROMOTIONS",
            "CATEGORY_SOCIAL",
            "CATEGORY_UPDATES",
            "CHAT",
            "DRAFT",
            "IMPORTANT",
            "INBOX",
            "SENT",
            "SPAM",
            "STARRED",
            "TRASH",
            "UNREAD",
        ];
        Self {
            draft_present: labels.iter().any(|label| label == "DRAFT"),
            total: labels.len(),
            system: SYSTEM
                .iter()
                .copied()
                .filter(|known| labels.iter().any(|label| label == known))
                .collect(),
            custom: labels
                .iter()
                .filter(|label| label.starts_with("Label_"))
                .count(),
            unknown: labels
                .iter()
                .filter(|label| !SYSTEM.contains(&label.as_str()) && !label.starts_with("Label_"))
                .count(),
        }
    }
}

impl DraftListError {
    pub(crate) fn temporary_label_line(&self) -> Option<String> {
        self.temporary_labels.as_ref().map(|labels| format!(
            "arqen TEMPORARY_draft_label_failure draft_present={} total={} system=[{}] custom={} unknown={}",
            labels.draft_present, labels.total, labels.system.join(","), labels.custom, labels.unknown
        ))
    }

    // Explicit internal log contract: every text field is a finite enum, not
    // a provider value or retained error. Never use this in a public response.
    pub(crate) fn diagnostic_line(&self) -> String {
        let status = self
            .status
            .map(|s| s.as_u16().to_string())
            .unwrap_or_else(|| "unobserved".into());
        format!(
            "arqen draft_list_failure stage={:?} status={status} category={:?} reason={:?}",
            self.stage, self.category, self.reason
        )
    }
}

impl fmt::Display for DraftListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Gmail could not complete the draft page; no partial page was returned")
    }
}

impl std::error::Error for DraftListError {}

fn listing_error(
    stage: DraftListStage,
    category: DraftListCategory,
    status: Option<StatusCode>,
) -> anyhow::Error {
    let reason = match category {
        DraftListCategory::Transport => DraftListReason::Transport,
        DraftListCategory::ProviderStatus => DraftListReason::ProviderStatus,
        DraftListCategory::Decoding => DraftListReason::InvalidJsonShape,
        DraftListCategory::Validation => DraftListReason::InvalidReferenceId,
        DraftListCategory::ResponseTooLarge => DraftListReason::ResponseTooLarge,
        DraftListCategory::Configuration => DraftListReason::Configuration,
    };
    DraftListError {
        stage,
        category,
        status,
        reason,
        temporary_labels: None,
    }
    .into()
}

fn listing_invariant(
    reason: DraftListReason,
    status: StatusCode,
    temporary_labels: Option<TemporaryDraftLabels>,
) -> anyhow::Error {
    DraftListError {
        stage: DraftListStage::Detail,
        category: DraftListCategory::Validation,
        status: Some(status),
        reason,
        temporary_labels,
    }
    .into()
}

fn listing_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
    stage: DraftListStage,
) -> Result<T> {
    let status = response.status();
    parse_json_response(response).map_err(|error| {
        let category = if !status.is_success() {
            DraftListCategory::ProviderStatus
        } else if error.is::<serde_json::Error>() {
            DraftListCategory::Decoding
        } else if error.is::<std::io::Error>() {
            DraftListCategory::Transport
        } else {
            // The bounded parser's only remaining error is its response cap.
            DraftListCategory::ResponseTooLarge
        };
        let reason = if let Some(decoding) = error.downcast_ref::<serde_json::Error>() {
            if decoding.is_data() {
                // Inspect only a fixed serde prefix, then discard the error.
                // Never retain/log its text, which can contain provider values.
                if decoding.to_string().starts_with("missing field `") {
                    DraftListReason::MissingRequiredField
                } else {
                    DraftListReason::InvalidJsonShape
                }
            } else {
                DraftListReason::InvalidJsonSyntax
            }
        } else {
            match category {
                DraftListCategory::ProviderStatus => DraftListReason::ProviderStatus,
                DraftListCategory::Transport => DraftListReason::Transport,
                _ => DraftListReason::ResponseTooLarge,
            }
        };
        let diagnostic = DraftListError {
            stage,
            category,
            status: Some(status),
            reason,
            temporary_labels: None,
        };
        if category == DraftListCategory::ProviderStatus {
            // Retain only the sanitized status error for existing refresh and
            // broker mappings; never retain a transport or decoding source.
            anyhow::Error::new(GmailApiError { status }).context(diagnostic)
        } else {
            diagnostic.into()
        }
    })
}

#[derive(Deserialize)]
struct DraftResource {
    id: String,
    message: DraftMessage,
}

#[derive(Deserialize)]
struct DraftMessage {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    payload: Option<MessagePayload>,
}

#[derive(Deserialize)]
struct DraftListResource {
    #[serde(default)]
    drafts: Vec<DraftReference>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    result_size_estimate: Option<u32>,
}

#[derive(Deserialize)]
struct DraftReference {
    id: String,
}

#[derive(Deserialize)]
struct SentMessage {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
}

impl GmailApi {
    pub fn list_drafts(
        &self,
        access_token: &str,
        target_email: &str,
        request: ListDraftsRequest,
    ) -> Result<DraftListResponse> {
        let request = request.validate()?;
        let mut query = vec![
            ("maxResults", request.max_results.to_string()),
            (
                "fields",
                "drafts(id),nextPageToken,resultSizeEstimate".to_owned(),
            ),
        ];
        if let Some(token) = request.page_token {
            query.push(("pageToken", token));
        }
        let response = self
            .client
            .get(self.base_url.join("users/me/drafts").map_err(|_| {
                listing_error(DraftListStage::List, DraftListCategory::Configuration, None)
            })?)
            .bearer_auth(access_token)
            .query(&query)
            .send()
            .map_err(|_| listing_error(DraftListStage::List, DraftListCategory::Transport, None))?;
        let list_status = response.status();
        let listed: DraftListResource = listing_response(response, DraftListStage::List)?;
        let mut drafts = Vec::with_capacity(listed.drafts.len());
        for reference in listed.drafts {
            crate::gmail::DraftIdRequest {
                draft_id: reference.id.clone(),
            }
            .validate()
            .map_err(|_| {
                listing_error(
                    DraftListStage::List,
                    DraftListCategory::Validation,
                    Some(list_status),
                )
            })?;
            let mut url = self.base_url.join("users/me/drafts").map_err(|_| {
                listing_error(
                    DraftListStage::Detail,
                    DraftListCategory::Configuration,
                    None,
                )
            })?;
            url.path_segments_mut()
                .map_err(|_| {
                    listing_error(
                        DraftListStage::Detail,
                        DraftListCategory::Configuration,
                        None,
                    )
                })?
                .push(&reference.id);
            let response = self
                .client
                .get(url)
                .bearer_auth(access_token)
                .query(&[
                    ("format", "metadata"),
                    (
                        "fields",
                        "id,message(id,threadId,labelIds,snippet,payload(headers(name,value)))",
                    ),
                ])
                .send()
                .map_err(|_| {
                    listing_error(DraftListStage::Detail, DraftListCategory::Transport, None)
                })?;
            let detail_status = response.status();
            let draft: DraftResource = listing_response(response, DraftListStage::Detail)?;
            if draft.id != reference.id {
                return Err(listing_invariant(
                    DraftListReason::ReferenceDetailMismatch,
                    detail_status,
                    None,
                ));
            }
            let temporary_labels = TemporaryDraftLabels::from_labels(&draft.message.label_ids);
            drafts.push(draft_summary(draft).map_err(|reason| {
                let labels = matches!(
                    reason,
                    DraftListReason::MissingDraftLabel | DraftListReason::UnsupportedDraftLabels
                )
                .then_some(temporary_labels);
                listing_invariant(reason, detail_status, labels)
            })?);
        }
        Ok(DraftListResponse {
            target_email: target_email.to_owned(),
            drafts,
            next_page_token: listed.next_page_token,
            result_size_estimate: listed.result_size_estimate,
        })
    }

    pub fn create_draft(
        &self,
        access_token: &str,
        request: CreateDraftRequest,
    ) -> Result<DraftCreateResult> {
        let request = request.validate()?;
        let raw = encode_message(
            &[
                ("To", request.to),
                ("Subject", mime_header(&request.subject)),
                ("MIME-Version", "1.0".into()),
                ("Content-Type", "text/plain; charset=UTF-8".into()),
                ("Content-Transfer-Encoding", "8bit".into()),
            ],
            &request.body,
        );
        self.create_draft_raw(access_token, raw, None)
    }

    pub fn create_reply_draft(
        &self,
        access_token: &str,
        request: CreateReplyDraftRequest,
    ) -> Result<DraftCreateResult> {
        let request = request.validate()?;
        let mut message_url = self.base_url.join("users/me/messages")?;
        message_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(&request.message_id);
        let original: MessageResource = self
            .client
            .get(message_url)
            .bearer_auth(access_token)
            .query(&[
                ("format", "full"),
                ("fields", "id,threadId,payload(headers(name,value))"),
            ])
            .send()
            .context("read source message for reply draft")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            !original.id.is_empty() && !original.thread_id.is_empty(),
            "Gmail returned an incomplete source message"
        );
        let headers = original
            .payload
            .map(|payload| payload.headers)
            .unwrap_or_default();
        let header = |name: &str| {
            headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .map(|h| h.value.clone())
        };
        let recipient = header("Reply-To")
            .or_else(|| header("From"))
            .ok_or_else(|| anyhow::anyhow!("source message has no reply recipient"))?;
        validate_provider_header(&recipient)?;
        let subject = header("Subject").unwrap_or_default();
        validate_provider_header(&subject)?;
        let subject = if subject.trim().to_ascii_lowercase().starts_with("re:") {
            subject
        } else {
            format!("Re: {subject}")
        };
        let mut message_headers = vec![
            ("To", recipient),
            ("Subject", mime_header(&subject)),
            ("MIME-Version", "1.0".into()),
            ("Content-Type", "text/plain; charset=UTF-8".into()),
            ("Content-Transfer-Encoding", "8bit".into()),
        ];
        let message_id = header("Message-ID")
            .ok_or_else(|| anyhow::anyhow!("source message has no Message-ID reply header"))?;
        validate_provider_header(&message_id)?;
        message_headers.push(("In-Reply-To", message_id.clone()));
        let references = header("References")
            .map_or(message_id.clone(), |prior| format!("{prior} {message_id}"));
        validate_provider_header(&references)?;
        message_headers.push(("References", references));
        let raw = encode_message(&message_headers, &request.body);
        self.create_draft_raw(access_token, raw, Some(original.thread_id))
    }

    fn create_draft_raw(
        &self,
        access_token: &str,
        raw: String,
        thread_id: Option<String>,
    ) -> Result<DraftCreateResult> {
        let mut message = serde_json::json!({ "raw": raw });
        if let Some(thread_id) = thread_id {
            message["threadId"] = serde_json::Value::String(thread_id);
        }
        let draft: DraftResource = self
            .client
            .post(self.base_url.join("users/me/drafts")?)
            .bearer_auth(access_token)
            .json(&serde_json::json!({"message": message}))
            .send()
            .context("create Gmail draft")
            .and_then(parse_json_response)?;
        validate_draft_resource(&draft)
            .map_err(|_| anyhow::anyhow!("Gmail returned an incomplete draft"))?;
        Ok(DraftCreateResult {
            draft_id: draft.id,
            message_id: draft.message.id,
            thread_id: draft.message.thread_id,
        })
    }

    pub fn draft_message_id(&self, access_token: &str, draft_id: &str) -> Result<String> {
        let mut url = self.base_url.join("users/me/drafts")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(draft_id);
        let draft: DraftResource = self
            .client
            .get(url)
            .bearer_auth(access_token)
            .query(&[("format", "minimal"), ("fields", "id,message(id,threadId)")])
            .send()
            .context("verify Gmail draft")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            draft.id == draft_id && !draft.message.id.is_empty(),
            "Gmail returned an invalid draft"
        );
        Ok(draft.message.id)
    }

    pub fn delete_draft(&self, access_token: &str, draft_id: &str) -> Result<()> {
        let mut url = self.base_url.join("users/me/drafts")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
            .push(draft_id);
        let response = self
            .client
            .delete(url)
            .bearer_auth(access_token)
            .send()
            .context("permanently delete Gmail draft")?;
        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::Error::new(GmailApiError { status }));
        }
        Ok(())
    }

    pub fn send_draft(&self, access_token: &str, draft_id: &str) -> Result<DraftCreateResult> {
        let response: SentMessage = self
            .client
            .post(self.base_url.join("users/me/drafts/send")?)
            .bearer_auth(access_token)
            .json(&serde_json::json!({"id": draft_id}))
            .send()
            .context("send Gmail draft")
            .and_then(parse_json_response)?;
        anyhow::ensure!(
            !response.id.is_empty() && !response.thread_id.is_empty(),
            "Gmail returned an incomplete sent-message response"
        );
        Ok(DraftCreateResult {
            draft_id: draft_id.into(),
            message_id: response.id,
            thread_id: response.thread_id,
        })
    }
}

fn draft_summary(draft: DraftResource) -> std::result::Result<DraftSummary, DraftListReason> {
    validate_draft_resource(&draft)?;
    let headers = draft.message.payload.as_ref().map(|p| &p.headers);
    let header = |name: &str| {
        headers
            .and_then(|hs| hs.iter().find(|h| h.name.eq_ignore_ascii_case(name)))
            .map(|h| h.value.clone())
    };
    if draft.message.label_ids.is_empty() {
        return Err(DraftListReason::MissingDraftLabel);
    }
    if draft.message.label_ids.iter().any(|label| label != "DRAFT") {
        return Err(DraftListReason::UnsupportedDraftLabels);
    }
    Ok(DraftSummary {
        draft_id: draft.id,
        message_id: draft.message.id,
        thread_id: draft.message.thread_id,
        to: header("To").into_iter().collect(),
        subject: header("Subject"),
        date: header("Date"),
        snippet: truncate_snippet(&draft.message.snippet).0,
    })
}

fn validate_draft_resource(draft: &DraftResource) -> std::result::Result<(), DraftListReason> {
    if draft.id.is_empty() {
        return Err(DraftListReason::EmptyDraftId);
    }
    if draft.message.id.is_empty() {
        return Err(DraftListReason::EmptyMessageId);
    }
    if draft.message.thread_id.is_empty() {
        return Err(DraftListReason::EmptyThreadId);
    }
    Ok(())
}

fn encode_message(headers: &[(&str, String)], body: &str) -> String {
    let mut message = String::new();
    for (name, value) in headers {
        message.push_str(name);
        message.push_str(": ");
        message.push_str(value);
        message.push_str("\r\n");
    }
    message.push_str("\r\n");
    message.push_str(
        &body
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', "\r\n"),
    );
    URL_SAFE_NO_PAD.encode(message.as_bytes())
}

fn mime_header(value: &str) -> String {
    if value.is_ascii() {
        value.to_owned()
    } else {
        format!("=?UTF-8?B?{}?=", STANDARD.encode(value.as_bytes()))
    }
}

fn validate_provider_header(value: &str) -> Result<()> {
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "source message contains an invalid reply header"
    );
    Ok(())
}

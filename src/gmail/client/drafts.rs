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
        let listed: DraftListResource = self
            .client
            .get(self.base_url.join("users/me/drafts")?)
            .bearer_auth(access_token)
            .query(&query)
            .send()
            .context("list Gmail drafts")
            .and_then(parse_json_response)?;
        let mut drafts = Vec::with_capacity(listed.drafts.len());
        for reference in listed.drafts {
            let mut url = self.base_url.join("users/me/drafts")?;
            url.path_segments_mut()
                .map_err(|_| anyhow::anyhow!("Gmail API base URL cannot accept path segments"))?
                .push(&reference.id);
            let draft: DraftResource = self
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
                .context("read Gmail draft metadata")
                .and_then(parse_json_response)?;
            drafts.push(draft_summary(draft)?);
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
        validate_draft_resource(&draft)?;
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

fn draft_summary(draft: DraftResource) -> Result<DraftSummary> {
    validate_draft_resource(&draft)?;
    let headers = draft.message.payload.as_ref().map(|p| &p.headers);
    let header = |name: &str| {
        headers
            .and_then(|hs| hs.iter().find(|h| h.name.eq_ignore_ascii_case(name)))
            .map(|h| h.value.clone())
    };
    anyhow::ensure!(
        draft.message.label_ids.is_empty()
            || draft.message.label_ids.iter().all(|label| label == "DRAFT"),
        "Gmail returned labels unsupported for a draft"
    );
    Ok(DraftSummary {
        draft_id: draft.id,
        message_id: draft.message.id,
        thread_id: draft.message.thread_id,
        to: header("To").into_iter().collect(),
        subject: header("Subject"),
        date: header("Date"),
        snippet: draft.message.snippet,
    })
}

fn validate_draft_resource(draft: &DraftResource) -> Result<()> {
    anyhow::ensure!(
        !draft.id.is_empty() && !draft.message.id.is_empty() && !draft.message.thread_id.is_empty(),
        "Gmail returned an incomplete draft"
    );
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

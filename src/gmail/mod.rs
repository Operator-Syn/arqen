// SPDX-License-Identifier: MPL-2.0
use anyhow::{Context, Result};
use base64::Engine;
use encoding_rs::{Encoding, UTF_8};
use reqwest::{StatusCode, blocking::Client};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fmt, io::Read, time::Duration};
use url::Url;

pub const GMAIL_API_BASE_URL: &str = "https://gmail.googleapis.com/gmail/v1/";
pub const DEFAULT_MAX_RESULTS: u32 = 20;
pub const MAX_MAX_RESULTS: u32 = 50;
pub const MAX_READ_EMAIL_BODY_BYTES: usize = 256 * 1024;
pub const MAX_READ_EMAIL_GMAIL_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_QUERY_LENGTH: usize = 1_024;
const MAX_PAGE_TOKEN_LENGTH: usize = 4_096;
const MAX_LABEL_IDS: usize = 20;
const MAX_LABEL_ID_LENGTH: usize = 256;
const MAX_SNIPPET_CHARS: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ListEmailsRequest {
    /// Gmail search syntax; control characters are not allowed. Omitted, null, or blank values use `in:inbox`.
    #[schemars(
        default = "default_query",
        length(max = 1_024),
        regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]*$")
    )]
    #[serde(default)]
    pub query: Option<String>,
    /// Gmail label IDs to match; control characters are not allowed. Defaults to an empty list; at most 20 IDs, each 1–256 characters.
    #[schemars(
        length(max = 20),
        inner(
            length(min = 1, max = 256),
            regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]*$")
        )
    )]
    #[serde(default)]
    pub label_ids: Vec<String>,
    /// Maximum number of message summaries to return. Defaults to 20; valid range is 1–50.
    #[schemars(range(min = 1, max = 50))]
    #[serde(default = "default_max_results")]
    pub max_results: u32,
    /// Opaque token from `next_page_token`; supply it to fetch the next page. It must be 1–4,096 characters without control characters.
    #[schemars(
        length(min = 1, max = 4_096),
        regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]*$")
    )]
    #[serde(default)]
    pub page_token: Option<String>,
    /// Whether to include messages from Gmail spam and trash. Defaults to false.
    #[serde(default)]
    pub include_spam_trash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ReadEmailRequest {
    /// The ID returned by `list_emails`. Supply only the message ID, not an account identifier.
    #[schemars(length(min = 1, max = 256), regex(pattern = r"^[A-Za-z0-9_-]+$"))]
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateLabelRequest {
    /// A nonblank custom Gmail label name. Gmail rejects names reserved for system labels.
    #[schemars(
        length(min = 1),
        regex(pattern = r"^(?=.*\S)[^\u0000-\u001F\u007F-\u009F]+$")
    )]
    pub name: String,
}

impl CreateLabelRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            !self.name.trim().is_empty(),
            "name must contain at least one non-whitespace character"
        );
        anyhow::ensure!(
            !self.name.chars().any(char::is_control),
            "name must not contain control characters"
        );
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DeleteLabelRequest {
    /// Exact Gmail label ID from list_labels; display names are not accepted.
    #[schemars(length(min = 1), regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$"))]
    pub label_id: String,
}

impl DeleteLabelRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(!self.label_id.is_empty(), "label_id cannot be empty");
        anyhow::ensure!(
            !self.label_id.chars().any(char::is_control),
            "label_id must not contain control characters"
        );
        Ok(self)
    }
}

impl ReadEmailRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            (1..=256).contains(&self.message_id.len())
                && self
                    .message_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'),
            "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
        );
        Ok(self)
    }
}

fn default_max_results() -> u32 {
    DEFAULT_MAX_RESULTS
}

fn default_query() -> Option<String> {
    Some("in:inbox".into())
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

include!("validation.rs");
include!("models.rs");
include!("client.rs");
include!("responses.rs");
include!("tests.rs");

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

include!("validation.rs");
include!("models.rs");
include!("client.rs");
include!("responses.rs");
include!("tests.rs");

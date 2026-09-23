use crate::secrets::{
    delete_refresh_token, load_refresh_token, store_refresh_token, token_reference,
};
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::Path, time::Duration};
use url::Url;
use uuid::Uuid;

#[cfg(test)]
const KEYRING_SERVICE: &str = "arqen";
const REDIRECT_URI: &str = "http://localhost";
const REVOCATION_URI: &str = "https://oauth2.googleapis.com/revoke";
pub const SUBJECT_MISMATCH_MESSAGE: &str =
    "Google account does not match the account selected for reauthentication";
const REQUESTED_SCOPES: &str = "openid email profile https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.modify";

#[derive(Debug, PartialEq, Eq)]
pub struct Callback {
    pub code: String,
    pub state: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct GoogleLogin {
    pub profile: GoogleProfile,
    pub granted_scopes: Vec<String>,
}

pub fn parse_callback(input: &str) -> Result<Callback> {
    let url = Url::parse(input.trim())?;
    let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        anyhow::bail!("Google authorization failed: {error}");
    }
    let code = params
        .get("code")
        .cloned()
        .context("redirect URL does not contain an authorization code")?;
    let state = params
        .get("state")
        .cloned()
        .context("redirect URL does not contain OAuth state")?;
    Ok(Callback { code, state })
}

#[derive(Debug, Deserialize)]
struct InstalledCredentials {
    client_id: String,
    client_secret: String,
    auth_uri: String,
    token_uri: String,
}

#[derive(Debug, Deserialize)]
struct CredentialsFile {
    installed: InstalledCredentials,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct RefreshedAccessToken {
    pub(crate) value: String,
    pub(crate) expires_in: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct GoogleTokenError {
    status: StatusCode,
    code: Option<String>,
    message: String,
}

#[cfg(test)]
impl GoogleTokenError {
    pub(crate) fn for_test(
        status: StatusCode,
        code: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status,
            code: code.map(str::to_owned),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GoogleTokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(code) = &self.code {
            write!(
                formatter,
                "Google token endpoint returned HTTP {} ({code}): {}",
                self.status, self.message
            )
        } else {
            write!(
                formatter,
                "Google token endpoint returned HTTP {}: {}",
                self.status, self.message
            )
        }
    }
}

impl std::error::Error for GoogleTokenError {}

#[derive(Debug, Deserialize)]
struct RevocationError {
    error: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GoogleProfile {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
}

pub struct GoogleOAuth {
    credentials: InstalledCredentials,
    client: Client,
    verifier: Option<String>,
    state: Option<String>,
    redirect_uri: String,
}

include!("oauth.rs");
include!("credentials.rs");
include!("tokens.rs");
include!("revocation.rs");
include!("scopes.rs");
include!("tests.rs");

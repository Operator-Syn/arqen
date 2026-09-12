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

const KEYRING_SERVICE: &str = "arqen";
const REDIRECT_URI: &str = "http://localhost";
const REVOCATION_URI: &str = "https://oauth2.googleapis.com/revoke";
pub const SUBJECT_MISMATCH_MESSAGE: &str =
    "Google account does not match the account selected for reauthentication";
const REQUESTED_SCOPES: &str =
    "openid email profile https://www.googleapis.com/auth/gmail.readonly";

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

impl GoogleOAuth {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let credentials = read_credentials(path)?;
        Ok(Self {
            credentials,
            client: Client::new(),
            verifier: None,
            state: None,
            redirect_uri: REDIRECT_URI.into(),
        })
    }

    pub fn set_redirect_uri(&mut self, redirect_uri: impl Into<String>) {
        self.redirect_uri = redirect_uri.into();
    }

    pub fn authorization_url(&mut self) -> Result<String> {
        let state = Uuid::new_v4().to_string();
        let verifier = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse(&self.credentials.auth_uri)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.credentials.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", REQUESTED_SCOPES)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        self.verifier = Some(verifier);
        self.state = Some(state);
        Ok(url.to_string())
    }

    pub fn finish(
        &mut self,
        callback: Callback,
        expected_subject: Option<&str>,
    ) -> Result<GoogleLogin> {
        let expected_state = self.state.take().context("no login is in progress")?;
        anyhow::ensure!(callback.state == expected_state, "OAuth state mismatch");
        let verifier = self
            .verifier
            .take()
            .context("no PKCE verifier is available")?;
        let token: TokenResponse = self
            .client
            .post(&self.credentials.token_uri)
            .form(&[
                ("code", callback.code.as_str()),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", self.credentials.client_secret.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .context("send authorization-code exchange to Google")?
            .error_for_status()
            .context("Google rejected the authorization-code exchange")?
            .json()
            .context("parse Google's token response")?;
        let granted_scopes = granted_scopes(token.scope.as_deref())?;
        let profile: GoogleProfile = self
            .client
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(&token.access_token)
            .send()
            .context("request Google account profile")?
            .error_for_status()
            .context("Google rejected the profile request")?
            .json()
            .context("parse Google's account profile")?;
        ensure_expected_subject(&profile, expected_subject)?;
        let refresh_token = token
            .refresh_token
            .context("Google did not return a refresh token; retry login with consent")?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &profile.sub)
            .context("create OS keyring entry for Google refresh token")?;
        entry
            .set_password(&refresh_token)
            .context("save Google refresh token in the OS keyring")?;
        Ok(GoogleLogin {
            profile,
            granted_scopes,
        })
    }
}

fn read_credentials(path: &Path) -> Result<InstalledCredentials> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read OAuth client JSON at {}", path.display()))?;
    let credentials: CredentialsFile = serde_json::from_str(&content)
        .with_context(|| format!("parse OAuth client JSON at {}", path.display()))?;
    Ok(credentials.installed)
}

pub(crate) fn refresh_google_access_token(
    credentials_path: impl AsRef<Path>,
    token_key: Option<&str>,
    subject: &str,
) -> Result<RefreshedAccessToken> {
    let credentials = read_credentials(credentials_path.as_ref())?;
    let (service, user) = keyring_coordinates(token_key, subject);
    let entry = keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?;
    let refresh_token = match entry.get_password() {
        Ok(refresh_token) => refresh_token,
        Err(keyring::Error::NoEntry) => {
            anyhow::bail!("no stored Google refresh token; reauthenticate the selected MCP target")
        }
        Err(error) => {
            return Err(anyhow::Error::new(error))
                .context("read Google refresh token from the OS keyring");
        }
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("create Google token refresh client")?;
    let response = client
        .post(&credentials.token_uri)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .context("send Google access-token refresh request")?;
    let token = parse_token_response(response)?;
    if let Some(rotated_refresh_token) = token.refresh_token {
        entry
            .set_password(&rotated_refresh_token)
            .context("save rotated Google refresh token in the OS keyring")?;
    }
    Ok(RefreshedAccessToken {
        value: token.access_token,
        expires_in: token.expires_in,
    })
}

fn parse_token_response(response: Response) -> Result<TokenResponse> {
    let status = response.status();
    if !status.is_success() {
        let error = response.json::<TokenErrorResponse>().unwrap_or_default();
        return Err(anyhow::Error::new(GoogleTokenError {
            status,
            code: error.error,
            message: error
                .error_description
                .unwrap_or_else(|| "the token request failed".into()),
        }));
    }
    let token: TokenResponse = response
        .json()
        .context("parse Google access-token response")?;
    anyhow::ensure!(
        !token.access_token.is_empty(),
        "Google returned an empty access token"
    );
    Ok(token)
}

#[derive(Debug, Default, Deserialize)]
struct TokenErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

pub(crate) fn is_invalid_grant(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<GoogleTokenError>()
        .and_then(|error| error.code.as_deref())
        == Some("invalid_grant")
}

pub(crate) fn is_missing_refresh_token(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("no stored Google refresh token"))
}

pub(crate) fn check_google_refresh_token(token_key: Option<&str>, subject: &str) -> Result<()> {
    let (service, user) = keyring_coordinates(token_key, subject);
    let entry = keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?;
    match entry.get_password() {
        Ok(refresh_token) => {
            anyhow::ensure!(
                !refresh_token.is_empty(),
                "stored Google refresh token is empty"
            );
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            anyhow::bail!("no stored Google refresh token")
        }
        Err(error) => {
            Err(anyhow::Error::new(error)).context("read Google refresh token from the OS keyring")
        }
    }
}

pub fn revoke_google_account(token_key: Option<&str>, subject: &str) -> Result<()> {
    let (service, user) = keyring_coordinates(token_key, subject);
    let entry = keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?;
    let refresh_token = match entry.get_password() {
        Ok(refresh_token) => refresh_token,
        Err(keyring::Error::NoEntry) => {
            anyhow::bail!(
                "no stored Google refresh token; reauthenticate this account before disconnecting"
            )
        }
        Err(error) => {
            return Err(anyhow::anyhow!(error))
                .context("read Google refresh token from the OS keyring");
        }
    };
    revoke_refresh_token(&refresh_token)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => {
            Err(anyhow::anyhow!(error)).context("remove Google refresh token from the OS keyring")
        }
    }
}

fn keyring_coordinates(token_key: Option<&str>, subject: &str) -> (String, String) {
    let Some(reference) = token_key.and_then(|value| value.strip_prefix("keyring:")) else {
        return (KEYRING_SERVICE.to_owned(), subject.to_owned());
    };
    let Some((service, user)) = reference.split_once(':') else {
        return (KEYRING_SERVICE.to_owned(), subject.to_owned());
    };
    if service.is_empty() || user.is_empty() {
        return (KEYRING_SERVICE.to_owned(), subject.to_owned());
    }
    (service.to_owned(), user.to_owned())
}

fn revoke_refresh_token(refresh_token: &str) -> Result<()> {
    let response = Client::new()
        .post(REVOCATION_URI)
        .form(&[("token", refresh_token)])
        .send()
        .context("send Google token revocation request")?;
    let status = response.status();
    let error_code = if status.is_success() {
        None
    } else {
        response
            .json::<RevocationError>()
            .ok()
            .and_then(|error| error.error)
    };
    validate_revocation_response(status, error_code.as_deref())
}

fn validate_revocation_response(status: StatusCode, error_code: Option<&str>) -> Result<()> {
    if status.is_success()
        || (status == StatusCode::BAD_REQUEST && error_code == Some("invalid_token"))
    {
        return Ok(());
    }
    match error_code {
        Some(error_code) => {
            anyhow::bail!("Google token revocation failed with HTTP {status} ({error_code})")
        }
        None => anyhow::bail!("Google token revocation failed with HTTP {status}"),
    }
}

fn canonical_scopes(raw: &str) -> Result<Vec<String>> {
    let mut scopes: Vec<String> = raw.split_whitespace().map(str::to_owned).collect();
    scopes.sort();
    scopes.dedup();
    anyhow::ensure!(
        !scopes.is_empty(),
        "Google returned an empty granted-scope set"
    );
    Ok(scopes)
}

fn granted_scopes(raw: Option<&str>) -> Result<Vec<String>> {
    raw.context("Google did not return granted scopes")
        .and_then(canonical_scopes)
}

fn ensure_expected_subject(profile: &GoogleProfile, expected_subject: Option<&str>) -> Result<()> {
    if let Some(expected_subject) = expected_subject {
        anyhow::ensure!(profile.sub == expected_subject, SUBJECT_MISMATCH_MESSAGE);
    }
    Ok(())
}

pub fn token_key(subject: &str) -> String {
    format!("keyring:{KEYRING_SERVICE}:{subject}")
}

#[cfg(test)]
mod tests {
    use super::{
        GoogleOAuth, GoogleProfile, InstalledCredentials, canonical_scopes,
        ensure_expected_subject, granted_scopes, parse_callback, validate_revocation_response,
    };
    use reqwest::StatusCode;
    use reqwest::blocking::Client;

    #[test]
    fn parses_google_redirect_url() {
        let callback =
            parse_callback("http://localhost/?code=abc123&scope=email&state=state-value").unwrap();
        assert_eq!(callback.code, "abc123");
        assert_eq!(callback.state, "state-value");
    }

    #[test]
    fn rejects_google_error_redirect() {
        let error = parse_callback("http://localhost/?error=access_denied&state=x").unwrap_err();
        assert!(error.to_string().contains("access_denied"));
    }

    #[test]
    fn authorization_url_uses_dynamic_redirect_uri() {
        let mut oauth = GoogleOAuth {
            credentials: InstalledCredentials {
                client_id: "client".into(),
                client_secret: "secret".into(),
                auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_uri: "https://oauth2.googleapis.com/token".into(),
            },
            client: Client::new(),
            verifier: None,
            state: None,
            redirect_uri: "http://localhost".into(),
        };
        oauth.set_redirect_uri("http://127.0.0.1:43123/oauth2/callback");
        let url = oauth.authorization_url().unwrap();
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43123%2Foauth2%2Fcallback"));
        assert!(url.contains(
            "scope=openid+email+profile+https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.readonly"
        ));
    }

    #[test]
    fn canonicalizes_and_deduplicates_granted_scopes() {
        assert_eq!(
            canonical_scopes("profile openid profile https://example.test/scope").unwrap(),
            vec!["https://example.test/scope", "openid", "profile"]
        );
    }

    #[test]
    fn rejects_empty_granted_scope_sets() {
        assert!(canonical_scopes(" \t\n").is_err());
    }

    #[test]
    fn rejects_missing_granted_scopes() {
        assert!(granted_scopes(None).is_err());
    }

    #[test]
    fn reads_granted_scopes_from_the_token_response() {
        let token: super::TokenResponse = serde_json::from_str(
            r#"{
                "access_token": "access-token",
                "refresh_token": "refresh-token",
                "scope": "profile openid"
            }"#,
        )
        .unwrap();
        assert_eq!(
            granted_scopes(token.scope.as_deref()).unwrap(),
            vec!["openid", "profile"]
        );
    }

    #[test]
    fn rejects_reauthentication_subject_mismatch() {
        let profile = GoogleProfile {
            sub: "returned-subject".into(),
            email: "returned@example.com".into(),
            name: None,
        };
        let error = ensure_expected_subject(&profile, Some("selected-subject")).unwrap_err();
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn treats_success_and_already_revoked_tokens_as_terminal() {
        assert!(validate_revocation_response(StatusCode::OK, None).is_ok());
        assert!(
            validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_token")).is_ok()
        );
    }

    #[test]
    fn rejects_other_revocation_failures_without_exposing_tokens() {
        let error = validate_revocation_response(StatusCode::BAD_REQUEST, Some("invalid_request"))
            .unwrap_err();
        assert!(error.to_string().contains("invalid_request"));
        assert!(!error.to_string().contains("refresh-token"));
    }

    #[test]
    fn uses_current_keyring_coordinates_for_missing_or_malformed_references() {
        assert_eq!(
            super::keyring_coordinates(None, "subject"),
            ("arqen".into(), "subject".into())
        );
        assert_eq!(
            super::keyring_coordinates(Some("keyring:legacy:subject"), "fallback"),
            ("legacy".into(), "subject".into())
        );
        assert_eq!(
            super::keyring_coordinates(Some("not-a-reference"), "fallback"),
            ("arqen".into(), "fallback".into())
        );
    }
}

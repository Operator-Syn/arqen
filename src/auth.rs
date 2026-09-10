use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::Path};
use url::Url;
use uuid::Uuid;

const KEYRING_SERVICE: &str = "arqen";
const REDIRECT_URI: &str = "http://localhost";
const SCOPES: &str = "openid email profile https://www.googleapis.com/auth/gmail.readonly";

#[derive(Debug, PartialEq, Eq)]
pub struct Callback {
    pub code: String,
    pub state: String,
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
}

#[derive(Debug, Deserialize)]
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
        let content = fs::read_to_string(path)
            .with_context(|| format!("read OAuth client JSON at {}", path.display()))?;
        let credentials: CredentialsFile = serde_json::from_str(&content)
            .with_context(|| format!("parse OAuth client JSON at {}", path.display()))?;
        Ok(Self {
            credentials: credentials.installed,
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
            .append_pair("scope", SCOPES)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        self.verifier = Some(verifier);
        self.state = Some(state);
        Ok(url.to_string())
    }

    pub fn finish(&mut self, callback: Callback) -> Result<GoogleProfile> {
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
        let refresh_token = token
            .refresh_token
            .context("Google did not return a refresh token; retry login with consent")?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &profile.sub)
            .context("create OS keyring entry for Google refresh token")?;
        entry
            .set_password(&refresh_token)
            .context("save Google refresh token in the OS keyring")?;
        Ok(profile)
    }
}

pub fn token_key(subject: &str) -> String {
    format!("keyring:{KEYRING_SERVICE}:{subject}")
}

#[cfg(test)]
mod tests {
    use super::{GoogleOAuth, InstalledCredentials, parse_callback};
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
    }
}

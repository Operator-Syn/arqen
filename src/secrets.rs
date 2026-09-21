//! Refresh-token storage backends.
//!
//! Native runs keep using the OS keyring. Docker runs select the OpenBao
//! backend with `ARQEN_SECRET_BACKEND=openbao`; the MCP process never talks to
//! either backend because it only talks to the credential broker.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{env, fs, path::Path};

const KEYRING_SERVICE: &str = "arqen";
const DEFAULT_OPENBAO_ADDR: &str = "http://openbao:8200";
const DEFAULT_OPENBAO_MOUNT: &str = "secret";
const DEFAULT_OPENBAO_PREFIX: &str = "arqen/google";
pub const OPENBAO_REFERENCE_PREFIX: &str = "openbao:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Keyring,
    OpenBao,
}

fn configured_backend() -> Result<Backend> {
    match env::var("ARQEN_SECRET_BACKEND")
        .unwrap_or_else(|_| "keyring".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "keyring" => Ok(Backend::Keyring),
        "openbao" => Ok(Backend::OpenBao),
        other => {
            anyhow::bail!("ARQEN_SECRET_BACKEND must be `keyring` or `openbao`, got `{other}`")
        }
    }
}

fn backend_for_reference(reference: Option<&str>) -> Result<Backend> {
    if reference.is_some_and(|value| value.starts_with(OPENBAO_REFERENCE_PREFIX)) {
        return Ok(Backend::OpenBao);
    }
    if reference.is_some_and(|value| value.starts_with("keyring:")) {
        return Ok(Backend::Keyring);
    }
    configured_backend()
}

/// Return the opaque reference persisted in SQLite for a newly logged-in
/// subject. The reference contains no credential value.
pub fn token_reference(subject: &str) -> String {
    match configured_backend().unwrap_or(Backend::Keyring) {
        Backend::Keyring => format!("keyring:{KEYRING_SERVICE}:{subject}"),
        Backend::OpenBao => format!("{OPENBAO_REFERENCE_PREFIX}{}/{subject}", openbao_prefix()),
    }
}

pub(crate) fn store_refresh_token(
    token_key: Option<&str>,
    subject: &str,
    refresh_token: &str,
) -> Result<()> {
    anyhow::ensure!(
        !refresh_token.is_empty(),
        "Google returned an empty refresh token"
    );
    match backend_for_reference(token_key)? {
        Backend::Keyring => keyring_store(token_key, subject, refresh_token),
        Backend::OpenBao => OpenBaoClient::from_env()?.put(token_key, subject, refresh_token),
    }
}

pub(crate) fn load_refresh_token(token_key: Option<&str>, subject: &str) -> Result<String> {
    match backend_for_reference(token_key)? {
        Backend::Keyring => keyring_load(token_key, subject),
        Backend::OpenBao => OpenBaoClient::from_env()?.get(token_key, subject),
    }
}

pub(crate) fn delete_refresh_token(token_key: Option<&str>, subject: &str) -> Result<()> {
    match backend_for_reference(token_key)? {
        Backend::Keyring => keyring_delete(token_key, subject),
        Backend::OpenBao => OpenBaoClient::from_env()?.delete(token_key, subject),
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

fn keyring_store(token_key: Option<&str>, subject: &str, refresh_token: &str) -> Result<()> {
    let (service, user) = keyring_coordinates(token_key, subject);
    keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?
        .set_password(refresh_token)
        .context("save Google refresh token in the OS keyring")
}

fn keyring_load(token_key: Option<&str>, subject: &str) -> Result<String> {
    let (service, user) = keyring_coordinates(token_key, subject);
    let entry = keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?;
    match entry.get_password() {
        Ok(refresh_token) if !refresh_token.is_empty() => Ok(refresh_token),
        Ok(_) => {
            anyhow::bail!("no stored Google refresh token; reauthenticate the selected MCP target")
        }
        Err(keyring::Error::NoEntry) => {
            anyhow::bail!("no stored Google refresh token; reauthenticate the selected MCP target")
        }
        Err(error) => {
            Err(anyhow::Error::new(error)).context("read Google refresh token from the OS keyring")
        }
    }
}

fn keyring_delete(token_key: Option<&str>, subject: &str) -> Result<()> {
    let (service, user) = keyring_coordinates(token_key, subject);
    let entry = keyring::Entry::new(&service, &user)
        .context("create OS keyring entry for Google refresh token")?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(anyhow::Error::new(error))
            .context("remove Google refresh token from the OS keyring"),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OpenBaoClient {
    client: Client,
    address: String,
    mount: String,
    prefix: String,
    role_id: String,
    secret_id: String,
}

#[derive(Debug, Deserialize)]
struct AppRoleLoginResponse {
    auth: Option<AppRoleAuth>,
}

#[derive(Debug, Deserialize)]
struct AppRoleAuth {
    client_token: String,
}

#[derive(Debug, Deserialize)]
struct KvReadResponse {
    data: Option<KvReadData>,
}

#[derive(Debug, Deserialize)]
struct KvReadData {
    data: Option<KvSecretData>,
}

#[derive(Debug, Deserialize)]
struct KvSecretData {
    refresh_token: Option<String>,
}

impl OpenBaoClient {
    pub(crate) fn from_env() -> Result<Self> {
        let role_id = read_secret_file(
            env::var_os("ARQEN_OPENBAO_ROLE_ID_FILE")
                .context("set ARQEN_OPENBAO_ROLE_ID_FILE for the OpenBao backend")?,
            "OpenBao role ID",
        )?;
        let secret_id = read_secret_file(
            env::var_os("ARQEN_OPENBAO_SECRET_ID_FILE")
                .context("set ARQEN_OPENBAO_SECRET_ID_FILE for the OpenBao backend")?,
            "OpenBao secret ID",
        )?;
        let address = env::var("ARQEN_OPENBAO_ADDR")
            .unwrap_or_else(|_| DEFAULT_OPENBAO_ADDR.into())
            .trim_end_matches('/')
            .to_owned();
        let mount =
            env::var("ARQEN_OPENBAO_MOUNT").unwrap_or_else(|_| DEFAULT_OPENBAO_MOUNT.into());
        let prefix = openbao_prefix();
        anyhow::ensure!(!address.is_empty(), "ARQEN_OPENBAO_ADDR cannot be empty");
        anyhow::ensure!(!mount.is_empty(), "ARQEN_OPENBAO_MOUNT cannot be empty");
        Ok(Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .context("create OpenBao client")?,
            address,
            mount,
            prefix,
            role_id,
            secret_id,
        })
    }

    #[cfg(test)]
    fn for_test(address: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            address: address.into().trim_end_matches('/').to_owned(),
            mount: DEFAULT_OPENBAO_MOUNT.into(),
            prefix: DEFAULT_OPENBAO_PREFIX.into(),
            role_id: "role".into(),
            secret_id: "secret".into(),
        }
    }

    fn login(&self) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/v1/auth/approle/login", self.address))
            .json(&serde_json::json!({
                "role_id": self.role_id,
                "secret_id": self.secret_id,
            }))
            .send()
            .context("authenticate to OpenBao with AppRole")?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("OpenBao AppRole authentication failed with HTTP {status}");
        }
        let payload: AppRoleLoginResponse =
            response.json().context("parse OpenBao AppRole response")?;
        let token = payload
            .auth
            .map(|auth| auth.client_token)
            .filter(|token| !token.is_empty())
            .context("OpenBao AppRole response did not contain a client token")?;
        Ok(token)
    }

    fn secret_path(&self, token_key: Option<&str>, subject: &str) -> Result<String> {
        let path = token_key
            .and_then(|reference| reference.strip_prefix(OPENBAO_REFERENCE_PREFIX))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{}/{}", self.prefix, subject));
        validate_path(&path)?;
        Ok(path)
    }

    fn get(&self, token_key: Option<&str>, subject: &str) -> Result<String> {
        let token = self.login()?;
        let path = self.secret_path(token_key, subject)?;
        let response = self
            .client
            .get(format!("{}/v1/{}/data/{}", self.address, self.mount, path))
            .header("X-Vault-Token", token)
            .send()
            .context("read Google refresh token from OpenBao")?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("no stored Google refresh token; reauthenticate the selected MCP target");
        }
        if !status.is_success() {
            anyhow::bail!("OpenBao secret read failed with HTTP {status}");
        }
        let payload: KvReadResponse = response.json().context("parse OpenBao secret response")?;
        payload
            .data
            .and_then(|data| data.data)
            .and_then(|data| data.refresh_token)
            .filter(|token| !token.is_empty())
            .context("OpenBao secret did not contain a refresh token")
    }

    fn put(&self, token_key: Option<&str>, subject: &str, refresh_token: &str) -> Result<()> {
        let token = self.login()?;
        let path = self.secret_path(token_key, subject)?;
        let response = self
            .client
            .post(format!("{}/v1/{}/data/{}", self.address, self.mount, path))
            .header("X-Vault-Token", token)
            .json(&serde_json::json!({
                "data": {"refresh_token": refresh_token},
            }))
            .send()
            .context("write Google refresh token to OpenBao")?;
        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "OpenBao secret write failed with HTTP {status}"
        );
        Ok(())
    }

    fn delete(&self, token_key: Option<&str>, subject: &str) -> Result<()> {
        let token = self.login()?;
        let path = self.secret_path(token_key, subject)?;
        let response = self
            .client
            .delete(format!(
                "{}/v1/{}/metadata/{}",
                self.address, self.mount, path
            ))
            .header("X-Vault-Token", token)
            .send()
            .context("delete Google refresh token from OpenBao")?;
        let status = response.status();
        anyhow::ensure!(
            status.is_success() || status == reqwest::StatusCode::NOT_FOUND,
            "OpenBao secret deletion failed with HTTP {status}"
        );
        Ok(())
    }
}

fn read_secret_file(path: impl AsRef<Path>, name: &str) -> Result<String> {
    let path = path.as_ref();
    let value = fs::read_to_string(path)
        .with_context(|| format!("read {name} file at {}", path.display()))?;
    let value = value.trim().to_owned();
    anyhow::ensure!(!value.is_empty(), "{name} file is empty");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains control characters"
    );
    Ok(value)
}

fn openbao_prefix() -> String {
    env::var("ARQEN_OPENBAO_PREFIX").unwrap_or_else(|_| DEFAULT_OPENBAO_PREFIX.into())
}

fn validate_path(path: &str) -> Result<()> {
    anyhow::ensure!(!path.is_empty(), "OpenBao secret path cannot be empty");
    anyhow::ensure!(
        path.split('/').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        }),
        "OpenBao secret path contains unsupported characters"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OPENBAO_REFERENCE_PREFIX, OpenBaoClient, validate_path};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn mock_openbao(responses: Vec<(u16, &'static str)>) -> String {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let reason = match status {
                    200 => "OK",
                    204 => "No Content",
                    403 => "Forbidden",
                    404 => "Not Found",
                    _ => "Error",
                };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        address
    }

    #[test]
    fn openbao_references_are_opaque_paths() {
        assert_eq!(
            format!("{OPENBAO_REFERENCE_PREFIX}secret/arqen/google/subject"),
            "openbao:secret/arqen/google/subject"
        );
        assert!(validate_path("secret/arqen/google/subject").is_ok());
        assert!(validate_path("secret/arqen/google/subject with spaces").is_err());
        assert!(validate_path("secret//subject").is_err());
    }

    #[test]
    fn openbao_client_round_trips_kv_operations() {
        let address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (204, "{}"),
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (
                200,
                r#"{"data":{"data":{"refresh_token":"refresh-value"}}}"#,
            ),
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (204, "{}"),
        ]);
        let client = OpenBaoClient::for_test(address);
        client
            .put(
                Some("openbao:arqen/google/subject"),
                "subject",
                "refresh-value",
            )
            .unwrap();
        assert_eq!(
            client
                .get(Some("openbao:arqen/google/subject"), "subject")
                .unwrap(),
            "refresh-value"
        );
        client
            .delete(Some("openbao:arqen/google/subject"), "subject")
            .unwrap();
    }

    #[test]
    fn openbao_client_maps_missing_and_forbidden_secrets() {
        let missing_address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (404, "{}"),
        ]);
        let missing = OpenBaoClient::for_test(missing_address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("no stored Google refresh token")
        );

        let forbidden_address = mock_openbao(vec![
            (200, r#"{"auth":{"client_token":"client"}}"#),
            (403, "{}"),
        ]);
        let forbidden = OpenBaoClient::for_test(forbidden_address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert!(forbidden.to_string().contains("HTTP 403"));
    }

    #[test]
    fn openbao_client_rejects_malformed_auth_response() {
        let address = mock_openbao(vec![(200, "not-json")]);
        let error = OpenBaoClient::for_test(address)
            .get(Some("openbao:arqen/google/subject"), "subject")
            .unwrap_err();
        assert!(error.to_string().contains("parse OpenBao AppRole response"));
    }
}

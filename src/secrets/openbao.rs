// SPDX-License-Identifier: MPL-2.0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum OpenBaoFailure {
    Unavailable,
    AppRoleRejected,
    LoginFailed(u16),
    CredentialReadFailed(u16),
    CredentialWriteFailed(u16),
    CredentialDeleteFailed(u16),
    InvalidResponse,
}

impl std::fmt::Display for OpenBaoFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str(
                "OpenBao is unavailable; Arqen could not reach the protected credential store",
            ),
            Self::AppRoleRejected => {
                formatter.write_str("OpenBao rejected Arqen's AppRole credentials")
            }
            Self::LoginFailed(status) => write!(
                formatter,
                "OpenBao could not authenticate Arqen's service role (HTTP {status})"
            ),
            Self::CredentialReadFailed(status) => write!(
                formatter,
                "OpenBao could not read the protected Google credential (HTTP {status})"
            ),
            Self::CredentialWriteFailed(status) => write!(
                formatter,
                "OpenBao could not save the protected Google credential (HTTP {status})"
            ),
            Self::CredentialDeleteFailed(status) => write!(
                formatter,
                "OpenBao could not delete the protected Google credential (HTTP {status})"
            ),
            Self::InvalidResponse => {
                formatter.write_str("OpenBao returned an invalid protected-credential response")
            }
        }
    }
}

impl std::error::Error for OpenBaoFailure {}

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
            .map_err(|_| OpenBaoFailure::Unavailable)?;
        let status = response.status();
        if !status.is_success() {
            if status == reqwest::StatusCode::BAD_REQUEST {
                return Err(OpenBaoFailure::AppRoleRejected.into());
            }
            if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(OpenBaoFailure::Unavailable.into());
            }
            return Err(OpenBaoFailure::LoginFailed(status.as_u16()).into());
        }
        let payload: AppRoleLoginResponse =
            response.json().map_err(|_| OpenBaoFailure::InvalidResponse)?;
        let token = payload
            .auth
            .map(|auth| auth.client_token)
            .filter(|token| !token.is_empty())
            .ok_or(OpenBaoFailure::InvalidResponse)?;
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
            .map_err(|_| OpenBaoFailure::Unavailable)?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("no stored Google refresh token; reauthenticate the selected MCP target");
        }
        if !status.is_success() {
            return Err(OpenBaoFailure::CredentialReadFailed(status.as_u16()).into());
        }
        let payload: KvReadResponse = response
            .json()
            .map_err(|_| OpenBaoFailure::InvalidResponse)?;
        payload
            .data
            .and_then(|data| data.data)
            .and_then(|data| data.refresh_token)
            .filter(|token| !token.is_empty())
            .ok_or(OpenBaoFailure::InvalidResponse.into())
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
            .map_err(|_| OpenBaoFailure::Unavailable)?;
        let status = response.status();
        if !status.is_success() {
            return Err(OpenBaoFailure::CredentialWriteFailed(status.as_u16()).into());
        }
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
            .map_err(|_| OpenBaoFailure::Unavailable)?;
        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::NOT_FOUND {
            return Err(OpenBaoFailure::CredentialDeleteFailed(status.as_u16()).into());
        }
        Ok(())
    }
}

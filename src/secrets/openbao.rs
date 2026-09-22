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

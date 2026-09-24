// SPDX-License-Identifier: MPL-2.0
impl ServerOptions {
    pub fn from_env(default_broker_socket: PathBuf) -> anyhow::Result<Self> {
        let listen_addr = std::env::var("ARQEN_MCP_LISTEN_ADDR")
            .unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.into())
            .parse()
            .context("parse ARQEN_MCP_LISTEN_ADDR")?;
        let broker_socket = std::env::var_os("ARQEN_GMAIL_BROKER_SOCKET")
            .map(PathBuf::from)
            .unwrap_or(default_broker_socket);
        let allowed_hosts = split_list_env("ARQEN_MCP_ALLOWED_HOSTS")?;
        let allowed_origins = split_list_env("ARQEN_MCP_ALLOWED_ORIGINS")?;
        anyhow::ensure!(
            !allowed_hosts.is_empty(),
            "ARQEN_MCP_ALLOWED_HOSTS must contain at least one public Host value"
        );
        anyhow::ensure!(
            !allowed_origins.is_empty(),
            "ARQEN_MCP_ALLOWED_ORIGINS must contain at least one allowed Origin"
        );
        let bearer_token = bearer_token_from_env()?;
        validate_secret(&bearer_token, "MCP bearer token")?;
        Ok(Self {
            listen_addr,
            broker_socket,
            allowed_hosts,
            allowed_origins,
            bearer_token,
        })
    }
}

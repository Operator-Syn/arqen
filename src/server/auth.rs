fn split_list_env(name: &str) -> anyhow::Result<Vec<String>> {
    let raw = std::env::var(name).with_context(|| format!("set {name}"))?;
    split_values(name, &raw)
}

fn split_values(name: &str, raw: &str) -> anyhow::Result<Vec<String>> {
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    anyhow::ensure!(
        !values
            .iter()
            .any(|value| value.chars().any(char::is_control)),
        "{name} contains control characters"
    );
    Ok(values)
}

fn bearer_token_from_env() -> anyhow::Result<String> {
    match std::env::var("ARQEN_MCP_BEARER_TOKEN") {
        Ok(token) => Ok(token),
        Err(std::env::VarError::NotPresent) => {
            let path = std::env::var_os("ARQEN_MCP_BEARER_TOKEN_FILE")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| {
                    crate::config::config_directory()
                        .map(|directory| directory.join("mcp-bearer-token"))
                })?;
            let content = std::fs::read_to_string(&path).with_context(|| {
                format!(
                    "read MCP bearer token file at {}",
                    PathBuf::from(&path).display()
                )
            })?;
            Ok(content
                .strip_suffix("\r\n")
                .or_else(|| content.strip_suffix('\n'))
                .unwrap_or(&content)
                .to_owned())
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("ARQEN_MCP_BEARER_TOKEN is not valid UTF-8")
        }
    }
}

fn validate_secret(value: &str, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!value.is_empty(), "{name} cannot be empty");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}

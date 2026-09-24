// SPDX-License-Identifier: MPL-2.0
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

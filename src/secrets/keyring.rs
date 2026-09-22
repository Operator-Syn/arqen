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

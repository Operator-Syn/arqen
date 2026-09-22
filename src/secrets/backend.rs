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

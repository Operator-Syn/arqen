// SPDX-License-Identifier: MPL-2.0
pub fn revoke_google_account(token_key: Option<&str>, subject: &str) -> Result<()> {
    let refresh_token = load_refresh_token(token_key, subject).context(
        "no stored Google refresh token; reauthenticate this account before disconnecting",
    )?;
    revoke_refresh_token(&refresh_token)?;
    delete_refresh_token(token_key, subject)
}

#[cfg(test)]
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

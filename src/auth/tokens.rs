// SPDX-License-Identifier: MPL-2.0
pub(crate) fn refresh_google_access_token(
    credentials_path: impl AsRef<Path>,
    token_key: Option<&str>,
    subject: &str,
) -> Result<RefreshedAccessToken> {
    let credentials = read_credentials(credentials_path.as_ref())?;
    let refresh_token = load_refresh_token(token_key, subject)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("create Google token refresh client")?;
    let response = client
        .post(&credentials.token_uri)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .context("send Google access-token refresh request")?;
    let token = parse_token_response(response)?;
    if let Some(rotated_refresh_token) = token.refresh_token {
        store_refresh_token(token_key, subject, &rotated_refresh_token)?;
    }
    Ok(RefreshedAccessToken {
        value: token.access_token,
        expires_in: token.expires_in,
    })
}

fn parse_token_response(response: Response) -> Result<TokenResponse> {
    let status = response.status();
    if !status.is_success() {
        let error = response.json::<TokenErrorResponse>().unwrap_or_default();
        return Err(anyhow::Error::new(GoogleTokenError {
            status,
            code: error.error,
            message: error
                .error_description
                .unwrap_or_else(|| "the token request failed".into()),
        }));
    }
    let token: TokenResponse = response
        .json()
        .context("parse Google access-token response")?;
    anyhow::ensure!(
        !token.access_token.is_empty(),
        "Google returned an empty access token"
    );
    Ok(token)
}

#[derive(Debug, Default, Deserialize)]
struct TokenErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

pub(crate) fn is_invalid_grant(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<GoogleTokenError>()
        .and_then(|error| error.code.as_deref())
        == Some("invalid_grant")
}

pub(crate) fn is_missing_refresh_token(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("no stored Google refresh token"))
}

pub(crate) fn check_google_refresh_token(token_key: Option<&str>, subject: &str) -> Result<()> {
    load_refresh_token(token_key, subject).map(|_| ())
}

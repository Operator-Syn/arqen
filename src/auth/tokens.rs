// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::net::{TcpStream, ToSocketAddrs};

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

/// Bound for the synthetic token-endpoint probe. Short enough that readiness
/// stays a fast probe; long enough for a cold lookup and connect.
const TOKEN_ENDPOINT_PROBE_TIMEOUT: Duration = Duration::from_secs(4);

/// Upper bound on resolved addresses tried before the endpoint is reported
/// unreachable, so a long address list cannot stretch a readiness probe.
const TOKEN_ENDPOINT_PROBE_ATTEMPTS: usize = 4;

/// Confirm the OAuth token endpoint a real refresh would use is reachable.
///
/// Readiness used to stop at "a refresh token is stored", which is a purely
/// local check: a broker whose container could not resolve DNS still reported
/// ready, and the next real refresh then failed with a transport error. This
/// probe resolves and connects to the configured `token_uri`, so that failure
/// class fails readiness instead of surfacing later as a failed tool call.
///
/// It deliberately sends no request and uses no credential. A real refresh on
/// every readiness poll would rotate the refresh token, mint throwaway access
/// tokens, and risk Google-side rate limiting; resolution plus the TCP connect
/// exercises the same DNS and egress path the refresh depends on.
pub(crate) fn probe_token_endpoint(credentials_path: impl AsRef<Path>) -> Result<()> {
    let credentials = read_credentials(credentials_path.as_ref())?;
    let endpoint =
        Url::parse(&credentials.token_uri).context("parse the configured OAuth token endpoint")?;
    let host = endpoint
        .host_str()
        .context("the configured OAuth token endpoint has no host")?;
    let port = endpoint
        .port_or_known_default()
        .context("the configured OAuth token endpoint has no port")?;
    let target = format!("{host}:{port}");

    let addresses = (host, port).to_socket_addrs().map_err(|error| {
        anyhow::Error::new(TokenEndpointUnreachable::new(
            target.clone(),
            format!("name resolution failed: {error}"),
        ))
    })?;

    let mut last_error = None;
    for address in addresses.take(TOKEN_ENDPOINT_PROBE_ATTEMPTS) {
        match TcpStream::connect_timeout(&address, TOKEN_ENDPOINT_PROBE_TIMEOUT) {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    let cause = match last_error {
        Some(error) => format!("connection failed: {error}"),
        None => "the endpoint resolved to no usable address".to_owned(),
    };
    Err(anyhow::Error::new(TokenEndpointUnreachable::new(
        target, cause,
    )))
}

pub(crate) fn is_token_endpoint_unreachable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<TokenEndpointUnreachable>().is_some()
}

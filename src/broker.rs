use crate::{
    Account, AccountStore, ConnectionState,
    auth::{
        check_google_refresh_token, is_invalid_grant, is_missing_refresh_token,
        refresh_google_access_token,
    },
    gmail::{EmailListResponse, GmailApi, GmailApiError, is_unauthorized},
    mcp::{BrokerErrorCode, BrokerFailure, BrokerRequest, BrokerResponse},
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_ACCESS_TOKEN_SECONDS: u64 = 3_600;
const ACCESS_TOKEN_SKEW_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
pub struct BrokerOptions {
    pub socket_path: PathBuf,
    pub database_path: PathBuf,
    pub credentials_path: PathBuf,
}

pub fn default_socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("set XDG_RUNTIME_DIR for the credential broker socket")?;
    anyhow::ensure!(
        !runtime_dir.is_empty(),
        "XDG_RUNTIME_DIR cannot be empty for the credential broker socket"
    );
    Ok(PathBuf::from(runtime_dir)
        .join("arqen")
        .join("gmail-broker.sock"))
}

#[derive(Debug, Clone)]
struct CachedAccessToken {
    value: String,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct BrokerState {
    database_path: PathBuf,
    credentials_path: PathBuf,
    access_tokens: Arc<Mutex<HashMap<String, CachedAccessToken>>>,
}

pub fn run(options: BrokerOptions) -> Result<()> {
    #[cfg(unix)]
    {
        run_unix(options)
    }
    #[cfg(not(unix))]
    {
        let _ = options;
        anyhow::bail!("the credential broker currently requires a Unix host")
    }
}

#[cfg(unix)]
fn run_unix(options: BrokerOptions) -> Result<()> {
    use signal_hook::{
        consts::{SIGINT, SIGTERM},
        flag,
    };
    use std::os::unix::net::UnixListener;

    prepare_socket_path(&options.socket_path)?;
    let listener = UnixListener::bind(&options.socket_path).with_context(|| {
        format!(
            "bind Arqen credential broker socket at {}",
            options.socket_path.display()
        )
    })?;
    restrict_socket_permissions(&options.socket_path)?;
    listener
        .set_nonblocking(true)
        .context("configure credential broker listener")?;
    let owned_socket_identity = socket_identity(&options.socket_path)?;
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGINT, Arc::clone(&shutdown)).context("register broker SIGINT handler")?;
    flag::register(SIGTERM, Arc::clone(&shutdown)).context("register broker SIGTERM handler")?;
    let state = BrokerState {
        database_path: options.database_path,
        credentials_path: options.credentials_path,
        access_tokens: Arc::new(Mutex::new(HashMap::new())),
    };
    let result = loop {
        if shutdown.load(Ordering::Relaxed) {
            break Ok(());
        }
        match listener.accept() {
            Ok(stream) => {
                let (stream, _) = stream;
                let state = state.clone();
                std::thread::Builder::new()
                    .name("arqen-gmail-broker-request".into())
                    .spawn(move || handle_connection(stream, &state))
                    .context("spawn credential broker request")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                break Err(error).context("accept credential broker connection");
            }
        }
    };
    if socket_identity(&options.socket_path).ok() == Some(owned_socket_identity) {
        let _ = fs::remove_file(&options.socket_path);
    }
    result
}

#[cfg(unix)]
fn prepare_socket_path(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create credential broker directory at {}", parent.display()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "restrict credential broker directory at {}",
            parent.display()
        )
    })?;
    if path.exists() {
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::net::UnixStream;

        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect credential broker path at {}", path.display()))?;
        anyhow::ensure!(
            metadata.file_type().is_socket(),
            "credential broker path already exists at {} and is not a Unix socket",
            path.display()
        );
        match UnixStream::connect(path) {
            Ok(_) => anyhow::bail!(
                "credential broker socket already exists at {}; stop the previous broker before starting another",
                path.display()
            ),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) =>
            {
                fs::remove_file(path).with_context(|| {
                    format!(
                        "remove stale credential broker socket at {}",
                        path.display()
                    )
                })?;
            }
            Err(error) => {
                anyhow::bail!(
                    "cannot safely determine whether credential broker socket {} is active: {error}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn socket_identity(path: &Path) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)
        .with_context(|| format!("inspect credential broker socket at {}", path.display()))?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
fn restrict_socket_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("restrict credential broker socket at {}", path.display()))
}

#[cfg(unix)]
fn handle_connection(stream: std::os::unix::net::UnixStream, state: &BrokerState) {
    let mut reader = BufReader::new(stream);
    let response = match read_request(&mut reader) {
        Ok(request) if request.operation == "readiness" => match request.validate_readiness() {
            Ok(()) => handle_readiness(state),
            Err(_) => BrokerResponse::error(
                BrokerErrorCode::InvalidRequest,
                "the broker request is invalid",
            ),
        },
        Ok(request) => match request.validate() {
            Ok(request) => handle_list_emails(request, state),
            Err(_) => BrokerResponse::error(
                BrokerErrorCode::InvalidRequest,
                "the broker request is invalid",
            ),
        },
        Err(_) => BrokerResponse::error(
            BrokerErrorCode::InvalidRequest,
            "the broker request could not be read",
        ),
    };
    let mut stream = reader.into_inner();
    if let Ok(encoded) = serde_json::to_vec(&response)
        && encoded.len() <= MAX_RESPONSE_BYTES
    {
        let _ = stream.write_all(&encoded);
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn handle_readiness(state: &BrokerState) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database is unavailable",
            );
        }
    };
    let target_subject = match store.mcp_configuration() {
        Ok(configuration) => match configuration.target_google_subject {
            Some(subject) => subject,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetNotConfigured,
                    "select an eligible MCP target account in Arqen first",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database configuration is unavailable",
            );
        }
    };
    let account = match store.list_accounts() {
        Ok(accounts) => match accounts
            .into_iter()
            .find(|account| account.subject == target_subject)
        {
            Some(account) => account,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetUnavailable,
                    "the configured MCP target account is no longer available",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database could not be read",
            );
        }
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    if check_google_refresh_token(account.token_key.as_deref(), &account.subject).is_err() {
        return BrokerResponse::error(
            BrokerErrorCode::TargetUnavailable,
            "the configured MCP target has no available local refresh credential",
        );
    }
    BrokerResponse::Ready
}

fn read_request<R: BufRead>(reader: &mut R) -> Result<BrokerRequest> {
    let line =
        read_bounded_line(reader, MAX_REQUEST_BYTES).context("read credential broker request")?;
    serde_json::from_slice(&line).context("parse credential broker request")
}

fn read_bounded_line<R: BufRead>(reader: &mut R, limit: usize) -> Result<Vec<u8>> {
    let mut line = Vec::with_capacity(limit.min(4096));
    loop {
        let available = reader.fill_buf().context("read bounded line")?;
        if available.is_empty() {
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        anyhow::ensure!(
            line.len().saturating_add(take) <= limit,
            "credential broker request is too large"
        );
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    anyhow::ensure!(!line.is_empty(), "credential broker request was empty");
    Ok(line)
}

fn handle_list_emails(
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database is unavailable",
            );
        }
    };
    let target_subject = match store.mcp_configuration() {
        Ok(configuration) => match configuration.target_google_subject {
            Some(subject) => subject,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetNotConfigured,
                    "select an eligible MCP target account in Arqen first",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database configuration is unavailable",
            );
        }
    };
    let account = match store.list_accounts() {
        Ok(accounts) => match accounts
            .into_iter()
            .find(|account| account.subject == target_subject)
        {
            Some(account) => account,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetUnavailable,
                    "the configured MCP target account is no longer available",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database could not be read",
            );
        }
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    let result = list_with_refresh(&account, request, state);
    match result {
        Ok(result) => BrokerResponse::Ok { result },
        Err(error) if is_invalid_grant(&error) || is_missing_refresh_token(&error) => {
            BrokerResponse::error(
                BrokerErrorCode::ReauthenticationRequired,
                "reauthenticate the selected MCP target account in Arqen",
            )
        }
        Err(error) => map_gmail_error(&error),
    }
}

fn target_ineligibility(account: &Account) -> Option<&'static str> {
    if account.connection_state != ConnectionState::Connected {
        return Some("the configured MCP target account is not connected");
    }
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return Some("the configured MCP target has unverified Google scopes");
    };
    if !scopes
        .iter()
        .any(|scope| scope == crate::GMAIL_READONLY_SCOPE)
    {
        return Some("the configured MCP target has no recorded Gmail read-only grant");
    }
    if account.token_key.is_none() {
        return Some("the configured MCP target has no protected keyring reference");
    }
    None
}

fn list_with_refresh(
    account: &Account,
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> Result<EmailListResponse> {
    let api = GmailApi::new()?;
    let token = cached_or_refresh_token(account, state)?;
    match api.list_emails(&token, &account.email, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            invalidate_token(&account.subject, state);
            let token = refresh_token(account, state)?;
            api.list_emails(&token, &account.email, request)
        }
        Err(error) => Err(error),
    }
}

fn cached_or_refresh_token(account: &Account, state: &BrokerState) -> Result<String> {
    if let Some(token) = state
        .access_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("access-token cache is unavailable"))?
        .get(&account.subject)
        .filter(|token| token.expires_at > Instant::now())
        .map(|token| token.value.clone())
    {
        return Ok(token);
    }
    refresh_token(account, state)
}

fn refresh_token(account: &Account, state: &BrokerState) -> Result<String> {
    let refreshed = refresh_google_access_token(
        &state.credentials_path,
        account.token_key.as_deref(),
        &account.subject,
    )?;
    let lifetime = refreshed
        .expires_in
        .unwrap_or(DEFAULT_ACCESS_TOKEN_SECONDS)
        .saturating_sub(ACCESS_TOKEN_SKEW_SECONDS)
        .max(30);
    let value = refreshed.value;
    state
        .access_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("access-token cache is unavailable"))?
        .insert(
            account.subject.clone(),
            CachedAccessToken {
                value: value.clone(),
                expires_at: Instant::now() + Duration::from_secs(lifetime),
            },
        );
    Ok(value)
}

fn invalidate_token(subject: &str, state: &BrokerState) {
    if let Ok(mut cache) = state.access_tokens.lock() {
        cache.remove(subject);
    }
}

fn map_gmail_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            429 => {
                return BrokerResponse::error(
                    BrokerErrorCode::GmailRateLimited,
                    "Gmail is rate limiting requests; try again shortly",
                );
            }
            _ => {}
        }
        return BrokerResponse::error(
            BrokerErrorCode::GmailUnavailable,
            "Gmail could not complete the mail-list request",
        );
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not complete the mail-list request",
    )
}

#[cfg(unix)]
#[derive(Clone)]
pub struct BrokerClient {
    socket_path: PathBuf,
}

#[cfg(unix)]
impl BrokerClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub async fn list_emails(
        &self,
        request: crate::gmail::ListEmailsRequest,
    ) -> std::result::Result<EmailListResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request =
            BrokerRequest::list_emails(request.validate().map_err(|_| BrokerFailure {
                code: BrokerErrorCode::InvalidRequest,
                message: "the mail-list request is invalid".into(),
            })?);
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response was invalid".into(),
            })?;
        match response {
            BrokerResponse::Ok { result } => Ok(result),
            BrokerResponse::Ready => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid mail-list response".into(),
            }),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
        }
    }

    pub async fn readiness(&self) -> std::result::Result<(), BrokerFailure> {
        match tokio::time::timeout(Duration::from_secs(2), self.readiness_inner()).await {
            Ok(result) => result,
            Err(_) => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker readiness check timed out".into(),
            }),
        }
    }

    async fn readiness_inner(&self) -> std::result::Result<(), BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request = BrokerRequest::readiness();
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker readiness request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker readiness request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness response was invalid".into(),
            })?;
        match response {
            BrokerResponse::Ready => Ok(()),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            BrokerResponse::Ok { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid readiness response".into(),
            }),
        }
    }
}

#[cfg(not(unix))]
#[derive(Clone)]
pub struct BrokerClient {
    _socket_path: PathBuf,
}

#[cfg(not(unix))]
impl BrokerClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            _socket_path: socket_path.into(),
        }
    }

    pub async fn list_emails(
        &self,
        _request: crate::gmail::ListEmailsRequest,
    ) -> std::result::Result<EmailListResponse, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn readiness(&self) -> std::result::Result<(), BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }
}

#[cfg(unix)]
async fn read_bounded_line_async<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncBufReadExt;

    let mut line = Vec::with_capacity(limit.min(4096));
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        if line.len().saturating_add(take) > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "credential broker response is too large",
            ));
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    if line.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "credential broker response was empty",
        ));
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::prepare_socket_path;
    use super::{read_bounded_line, target_ineligibility};
    use crate::gmail::GmailApiError;
    use crate::{Account, ConnectionState, GMAIL_READONLY_SCOPE};
    use reqwest::StatusCode;
    use std::io::BufReader;

    fn account() -> Account {
        Account {
            id: "account".into(),
            subject: "subject".into(),
            email: "user@example.com".into(),
            display_name: None,
            token_key: Some("keyring:arqen:subject".into()),
            granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into()]),
            connection_state: ConnectionState::Connected,
        }
    }

    #[test]
    fn target_eligibility_requires_connected_verified_gmail_access() {
        let eligible = account();
        assert_eq!(target_ineligibility(&eligible), None);
        let mut disconnected = eligible.clone();
        disconnected.connection_state = ConnectionState::Disconnected;
        assert!(target_ineligibility(&disconnected).is_some());
        let mut unverified = eligible.clone();
        unverified.granted_scopes = None;
        assert!(target_ineligibility(&unverified).is_some());
    }

    #[test]
    fn bounded_request_reader_rejects_unterminated_oversized_input() {
        let oversized = vec![b'x'; super::MAX_REQUEST_BYTES + 1];
        let error = read_bounded_line(
            &mut BufReader::new(oversized.as_slice()),
            super::MAX_REQUEST_BYTES,
        )
        .unwrap_err();
        assert!(error.to_string().contains("too large"));
    }

    #[test]
    fn bounded_request_reader_accepts_a_single_json_line() {
        let mut reader = BufReader::new(
            br#"{"operation":"list_emails"}
"#
            .as_slice(),
        );
        let line = read_bounded_line(&mut reader, super::MAX_REQUEST_BYTES).unwrap();
        assert_eq!(
            line,
            br#"{"operation":"list_emails"}
"#
        );
    }

    #[test]
    fn upstream_auth_and_rate_limit_failures_map_to_stable_broker_codes() {
        let unauthorized = super::map_gmail_error(&anyhow::Error::new(GmailApiError::for_test(
            StatusCode::UNAUTHORIZED,
            "token rejected",
        )));
        let limited = super::map_gmail_error(&anyhow::Error::new(GmailApiError::for_test(
            StatusCode::TOO_MANY_REQUESTS,
            "slow down",
        )));
        assert!(matches!(
            unauthorized,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::ReauthenticationRequired,
                ..
            }
        ));
        assert!(matches!(
            limited,
            crate::mcp::BrokerResponse::Error {
                code: crate::mcp::BrokerErrorCode::GmailRateLimited,
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn socket_setup_refuses_an_existing_path() {
        let path = std::env::temp_dir().join(format!(
            "arqen-broker-existing-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"not a socket").unwrap();
        let error = prepare_socket_path(&path).unwrap_err();
        assert!(error.to_string().contains("already exists"));
        let _ = std::fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn socket_setup_reclaims_a_stale_unix_socket() {
        use std::os::unix::net::UnixListener;

        let path = std::env::temp_dir().join(format!(
            "arqen-broker-stale-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        prepare_socket_path(&path).unwrap();
        assert!(!path.exists());
    }
}

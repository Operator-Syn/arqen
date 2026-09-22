use anyhow::{Context, Result, bail};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        Request, State,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::{any, get, post},
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    env, fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::{Child, Command},
    signal,
};
use tokio_tungstenite::{
    WebSocketStream, connect_async,
    tungstenite::{
        Message as UpstreamMessage, client::IntoClientRequest,
        protocol::CloseFrame as UpstreamCloseFrame,
    },
};
use uuid::Uuid;

use crate::ui::theme;

const DEFAULT_GATEWAY_ADDR: &str = "0.0.0.0:7681";
const DEFAULT_TTYD_ADDR: &str = "127.0.0.1:7682";
const DEFAULT_PASSWORD_FILE: &str = "/run/arqen-secrets/arqen-control-password";
const DEFAULT_TTYD_PATH: &str = "/usr/local/bin/ttyd";
const AUTH_HEADER: &str = "X-Arqen-Auth";
const AUTH_VALUE: &str = "1";
const SESSION_COOKIE: &str = "arqen_control_session";
const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_LOGIN_BODY_BYTES: usize = 4 * 1024;
const MAX_PROXY_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_FAILED_LOGINS: u8 = 5;
const FAILED_LOGIN_WINDOW: Duration = Duration::from_secs(60);

const LOGIN_PAGE_TEMPLATE: &str = include_str!("control/login.html");

#[derive(Debug, Clone)]
pub(crate) struct ControlGatewayOptions {
    pub(crate) listen_addr: SocketAddr,
    pub(crate) ttyd_addr: SocketAddr,
    pub(crate) password_file: PathBuf,
    pub(crate) ttyd_path: PathBuf,
    pub(crate) arqen_path: PathBuf,
}

impl ControlGatewayOptions {
    pub(crate) fn from_env() -> Result<Self> {
        let listen_addr = configured_addr("ARQEN_CONTROL_GATEWAY_ADDR", DEFAULT_GATEWAY_ADDR)?;
        let ttyd_addr = configured_addr("ARQEN_CONTROL_TTYD_ADDR", DEFAULT_TTYD_ADDR)?;
        anyhow::ensure!(
            ttyd_addr.ip().is_loopback(),
            "ARQEN_CONTROL_TTYD_ADDR must use a loopback address"
        );
        let password_file = env::var_os("ARQEN_CONTROL_PASSWORD_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PASSWORD_FILE));
        let ttyd_path = env::var_os("ARQEN_TTYD_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_TTYD_PATH));
        let arqen_path = env::current_exe().context("resolve the Arqen executable path")?;
        Ok(Self {
            listen_addr,
            ttyd_addr,
            password_file,
            ttyd_path,
            arqen_path,
        })
    }
}

fn configured_addr(name: &str, default: &str) -> Result<SocketAddr> {
    env::var(name)
        .unwrap_or_else(|_| default.into())
        .parse()
        .with_context(|| format!("parse {name}"))
}

#[derive(Clone)]
struct GatewayState {
    password: Arc<String>,
    sessions: Arc<SessionStore>,
    rate_limiter: Arc<Mutex<FailedLoginState>>,
    client: reqwest::Client,
    ttyd_http_url: String,
    ttyd_ws_url: String,
    ttyd_host: String,
}

impl GatewayState {
    fn new(password: String, ttyd_addr: SocketAddr) -> Result<Self> {
        validate_secret(&password, "control password")?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("build control gateway HTTP client")?;
        let ttyd_host = ttyd_addr.to_string();
        Ok(Self {
            password: Arc::new(password),
            sessions: Arc::new(SessionStore::default()),
            rate_limiter: Arc::new(Mutex::new(FailedLoginState::default())),
            client,
            ttyd_http_url: format!("http://{ttyd_host}"),
            ttyd_ws_url: format!("ws://{ttyd_host}"),
            ttyd_host,
        })
    }
}

#[derive(Default)]
struct SessionStore {
    sessions: Mutex<HashMap<String, Instant>>,
}

impl SessionStore {
    fn issue(&self) -> String {
        let token = Uuid::new_v4().simple().to_string();
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.retain(|_, expiry| *expiry > Instant::now());
        sessions.insert(token.clone(), Instant::now() + SESSION_TTL);
        token
    }

    fn contains(&self, token: &str) -> bool {
        let now = Instant::now();
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.retain(|_, expiry| *expiry > now);
        sessions.contains_key(token)
    }

    fn revoke(&self, token: &str) {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.remove(token);
    }
}

#[derive(Debug)]
struct FailedLoginState {
    window_started: Instant,
    failures: u8,
}

impl Default for FailedLoginState {
    fn default() -> Self {
        Self {
            window_started: Instant::now(),
            failures: 0,
        }
    }
}

impl FailedLoginState {
    fn retry_after(&mut self) -> Option<Duration> {
        let now = Instant::now();
        if now.duration_since(self.window_started) >= FAILED_LOGIN_WINDOW {
            self.window_started = now;
            self.failures = 0;
        }
        if self.failures >= MAX_FAILED_LOGINS {
            Some(FAILED_LOGIN_WINDOW.saturating_sub(now.duration_since(self.window_started)))
        } else {
            None
        }
    }

    fn record_failure(&mut self) {
        let _ = self.retry_after();
        self.failures = self.failures.saturating_add(1);
    }

    fn reset(&mut self) {
        self.window_started = Instant::now();
        self.failures = 0;
    }
}

pub(crate) fn run(options: ControlGatewayOptions) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create control gateway runtime")?;
    runtime.block_on(run_async(options))
}

async fn run_async(options: ControlGatewayOptions) -> Result<()> {
    let password = read_password_file(&options.password_file)?;
    let state = GatewayState::new(password, options.ttyd_addr)?;
    let ttyd = Arc::new(tokio::sync::Mutex::new(spawn_ttyd(&options).await?));
    let listener = match tokio::net::TcpListener::bind(options.listen_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            let mut ttyd = ttyd.lock().await;
            let _ = ttyd.kill().await;
            let _ = ttyd.wait().await;
            return Err(error)
                .with_context(|| format!("bind control gateway at {}", options.listen_addr));
        }
    };
    let ttyd_for_shutdown = Arc::clone(&ttyd);
    let ttyd_exited = Arc::new(AtomicBool::new(false));
    let ttyd_exited_by_shutdown = Arc::clone(&ttyd_exited);
    let shutdown = async move {
        tokio::select! {
            _ = shutdown_signal() => {},
            _ = async move {
                let mut ttyd = ttyd_for_shutdown.lock().await;
                let _ = ttyd.wait().await;
                ttyd_exited_by_shutdown.store(true, Ordering::Relaxed);
            } => {},
        }
    };
    let result = axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
        .context("serve Arqen control gateway");
    let mut ttyd = ttyd.lock().await;
    if ttyd.try_wait()?.is_none() {
        let _ = ttyd.kill().await;
        let _ = ttyd.wait().await;
    }
    if ttyd_exited.load(Ordering::Relaxed) && result.is_ok() {
        bail!("ttyd exited before the control gateway stopped")
    }
    result
}

async fn spawn_ttyd(options: &ControlGatewayOptions) -> Result<Child> {
    let port = options.ttyd_addr.port().to_string();
    Command::new(&options.ttyd_path)
        .args(["-W", "-O", "-i", "127.0.0.1", "-p", &port])
        .args(["-t", "rendererType=dom", "-H", AUTH_HEADER])
        .arg(&options.arqen_path)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("start ttyd from {}", options.ttyd_path.display()))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = signal::ctrl_c().await;
    }
}

fn build_router(state: GatewayState) -> Router {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/ws", get(websocket))
        .fallback(any(proxy_http))
        .with_state(state)
}

async fn login(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    {
        let mut limiter = state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(retry_after) = limiter.retry_after() {
            let mut response = login_response(
                StatusCode::TOO_MANY_REQUESTS,
                Some("Too many attempts. Wait a moment and try again."),
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after.as_secs().max(1).to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            return response;
        }
    }
    let body = match to_bytes(request.into_body(), MAX_LOGIN_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            state
                .rate_limiter
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .record_failure();
            return login_response(
                StatusCode::BAD_REQUEST,
                Some("Enter a username and password."),
            );
        }
    };
    let Some((username, password)) = parse_login_form(&body) else {
        state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record_failure();
        return login_response(
            StatusCode::UNAUTHORIZED,
            Some("Credentials not recognized."),
        );
    };
    let valid_username = username.as_bytes().ct_eq(b"arqen");
    let valid_password = password.as_bytes().ct_eq(state.password.as_bytes());
    if !bool::from(valid_username & valid_password) {
        state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record_failure();
        return login_response(
            StatusCode::UNAUTHORIZED,
            Some("Credentials not recognized."),
        );
    }
    state
        .rate_limiter
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .reset();
    let token = state.sessions.issue();
    redirect_with_cookie(StatusCode::SEE_OTHER, session_cookie(&token))
}

async fn logout(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if let Some(token) = session_token(request.headers()) {
        state.sessions.revoke(&token);
    }
    redirect_with_cookie(StatusCode::SEE_OTHER, expired_session_cookie())
}

async fn websocket(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    uri: Uri,
    mut upgrade: WebSocketUpgrade,
) -> Response {
    if !origin_is_allowed(&headers) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if !is_authenticated(&state, &headers) {
        return plain_response(StatusCode::UNAUTHORIZED);
    }
    let path = uri
        .path_and_query()
        .map(|value| value.as_str().to_owned())
        .unwrap_or_else(|| "/ws".into());
    let target = format!("{}{path}", state.ttyd_ws_url);
    let mut request = match target.into_client_request() {
        Ok(request) => request,
        Err(_) => return plain_response(StatusCode::SERVICE_UNAVAILABLE),
    };
    let Ok(host) = HeaderValue::from_str(&state.ttyd_host) else {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(origin) = HeaderValue::from_str(&format!("http://{}", state.ttyd_host)) else {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    request.headers_mut().insert(header::HOST, host);
    request.headers_mut().insert(header::ORIGIN, origin);
    request.headers_mut().insert(
        HeaderName::from_static("x-arqen-auth"),
        HeaderValue::from_static(AUTH_VALUE),
    );
    if let Some(protocols) = headers.get(header::SEC_WEBSOCKET_PROTOCOL) {
        request
            .headers_mut()
            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocols.clone());
    }
    let (upstream, upstream_response) =
        match tokio::time::timeout(Duration::from_secs(5), connect_async(request)).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) | Err(_) => return plain_response(StatusCode::SERVICE_UNAVAILABLE),
        };
    if let Some(protocol) = upstream_response
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
    {
        upgrade.set_selected_protocol(protocol.clone());
    } else if headers.contains_key(header::SEC_WEBSOCKET_PROTOCOL) {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    }
    upgrade
        .on_upgrade(move |socket| websocket_proxy(socket, upstream))
        .into_response()
}

async fn websocket_proxy<S>(client: WebSocket, upstream: WebSocketStream<S>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut upstream_sink, mut upstream_stream) = upstream.split();
    let (mut client_sink, mut client_stream) = client.split();
    let client_to_upstream = async {
        while let Some(Ok(message)) = client_stream.next().await {
            if upstream_sink
                .send(to_upstream_message(message))
                .await
                .is_err()
            {
                break;
            }
        }
    };
    let upstream_to_client = async {
        while let Some(Ok(message)) = upstream_stream.next().await {
            let Some(message) = from_upstream_message(message) else {
                continue;
            };
            if client_sink.send(message).await.is_err() {
                break;
            }
        }
    };
    tokio::pin!(client_to_upstream);
    tokio::pin!(upstream_to_client);
    tokio::select! {
        _ = &mut client_to_upstream => {},
        _ = &mut upstream_to_client => {},
    }
}

fn to_upstream_message(message: AxumMessage) -> UpstreamMessage {
    match message {
        AxumMessage::Text(text) => UpstreamMessage::Text(text.to_string().into()),
        AxumMessage::Binary(data) => UpstreamMessage::Binary(data),
        AxumMessage::Ping(data) => UpstreamMessage::Ping(data),
        AxumMessage::Pong(data) => UpstreamMessage::Pong(data),
        AxumMessage::Close(None) => UpstreamMessage::Close(None),
        AxumMessage::Close(Some(frame)) => UpstreamMessage::Close(Some(UpstreamCloseFrame {
            code: frame.code.into(),
            reason: frame.reason.to_string().into(),
        })),
    }
}

fn from_upstream_message(message: UpstreamMessage) -> Option<AxumMessage> {
    match message {
        UpstreamMessage::Text(text) => Some(AxumMessage::text(text.to_string())),
        UpstreamMessage::Binary(data) => Some(AxumMessage::binary(data)),
        UpstreamMessage::Ping(data) => Some(AxumMessage::Ping(data)),
        UpstreamMessage::Pong(data) => Some(AxumMessage::Pong(data)),
        UpstreamMessage::Close(_) => Some(AxumMessage::Close(None)),
        UpstreamMessage::Frame(_) => None,
    }
}

async fn proxy_http(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if !is_authenticated(&state, request.headers()) {
        if request.method() == Method::GET && request.uri().path() == "/" {
            return login_response(StatusCode::OK, None);
        }
        return plain_response(StatusCode::UNAUTHORIZED);
    }
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, MAX_PROXY_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => return plain_response(StatusCode::PAYLOAD_TOO_LARGE),
    };
    let path = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target = format!("{}{path}", state.ttyd_http_url);
    let method = match reqwest::Method::from_bytes(parts.method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return plain_response(StatusCode::BAD_REQUEST),
    };
    let mut builder = state
        .client
        .request(method, target)
        .header(AUTH_HEADER, AUTH_VALUE);
    for (name, value) in &parts.headers {
        if should_forward_request_header(name) {
            builder = builder.header(name, value);
        }
    }
    let response = match builder.body(body).send().await {
        Ok(response) => response,
        Err(_) => return plain_response(StatusCode::BAD_GATEWAY),
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let upstream_headers = response.headers().clone();
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(_) => return plain_response(StatusCode::BAD_GATEWAY),
    };
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name)
            && let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_ref()),
                HeaderValue::from_bytes(value.as_bytes()),
            )
        {
            response.headers_mut().insert(name, value);
        }
    }
    response
}

fn should_forward_request_header(name: &reqwest::header::HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "host"
            | "cookie"
            | "authorization"
            | "connection"
            | "upgrade"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "content-length"
            | "transfer-encoding"
            | "x-arqen-auth"
    )
}

fn should_forward_response_header(name: &reqwest::header::HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "set-cookie"
            | "www-authenticate"
    )
}

fn is_authenticated(state: &GatewayState, headers: &HeaderMap) -> bool {
    session_token(headers).is_some_and(|token| state.sessions.contains(&token))
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE && !value.is_empty()).then(|| value.to_owned())
    })
}

fn session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict",
        SESSION_TTL.as_secs()
    )
}

fn expired_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict")
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if origin == "null" {
        // Chromium uses an opaque Origin for this local form navigation; the
        // Fetch Metadata value keeps opaque cross-site requests rejected.
        return headers
            .get(HeaderName::from_static("sec-fetch-site"))
            .and_then(|value| value.to_str().ok())
            .is_some_and(|site| site.eq_ignore_ascii_case("same-origin"));
    }
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    origin == format!("http://{host}")
}

fn parse_login_form(body: &[u8]) -> Option<(String, String)> {
    let mut username = None;
    let mut password = None;
    for (key, value) in url::form_urlencoded::parse(body) {
        match key.as_ref() {
            "username" if username.is_none() => username = Some(value.into_owned()),
            "password" if password.is_none() => password = Some(value.into_owned()),
            "username" | "password" => return None,
            _ => {}
        }
    }
    Some((username?, password?))
}

fn validate_secret(value: &str, name: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "{name} cannot be empty");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}

fn read_password_file(path: &std::path::Path) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read control password file at {}", path.display()))?;
    let password = content
        .strip_suffix("\r\n")
        .or_else(|| content.strip_suffix('\n'))
        .unwrap_or(&content)
        .to_owned();
    validate_secret(&password, "control password")?;
    Ok(password)
}

fn login_response(status: StatusCode, error: Option<&str>) -> Response {
    let mut response = Html(login_page(error)).into_response();
    *response.status_mut() = status;
    set_page_headers(response.headers_mut());
    response
}

fn redirect_with_cookie(status: StatusCode, cookie: String) -> Response {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static("/"));
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    set_page_headers(response.headers_mut());
    response
}

fn plain_response(status: StatusCode) -> Response {
    let mut response = status.into_response();
    set_page_headers(response.headers_mut());
    response
}

fn set_page_headers(headers: &mut HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src data:; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
}

fn login_page(error: Option<&str>) -> String {
    let mut page = LOGIN_PAGE_TEMPLATE.to_owned();
    for (placeholder, value) in [
        ("__BACKGROUND__", theme::BACKGROUND_HEX),
        ("__SURFACE__", theme::SURFACE_HEX),
        ("__SURFACE_RAISED__", theme::SURFACE_RAISED_HEX),
        ("__PRIMARY__", theme::PRIMARY_HEX),
        ("__PRIMARY_STRONG__", theme::PRIMARY_STRONG_HEX),
        ("__TEXT__", theme::TEXT_HEX),
        ("__MUTED__", theme::MUTED_HEX),
        ("__SUCCESS__", theme::SUCCESS_HEX),
        ("__DANGER__", theme::DANGER_HEX),
        ("__BORDER__", theme::BORDER_HEX),
    ] {
        page = page.replace(placeholder, value);
    }
    let error = error.map(escape_html).unwrap_or_default();
    page.replace(
        "__ERROR_HIDDEN__",
        if error.is_empty() { "hidden" } else { "" },
    )
    .replace("__ERROR__", &error)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::any};
    use std::sync::Mutex as StdMutex;

    fn state(password: &str) -> GatewayState {
        GatewayState::new(password.into(), "127.0.0.1:7682".parse().unwrap()).unwrap()
    }

    fn request(body: &str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/auth/login")
            .header(header::HOST, "127.0.0.1:7681")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[test]
    fn login_page_uses_shared_theme_tokens_and_no_external_assets() {
        let page = login_page(None);
        assert!(page.contains("ARQEN"));
        assert!(page.contains(theme::BACKGROUND_HEX));
        assert!(page.contains(theme::PRIMARY_HEX));
        assert!(page.contains("form method=\"post\" action=\"/auth/login\""));
        assert!(page.contains("placeholder=\"arqen\""));
        assert!(!page.contains("value=\"arqen\""));
        assert!(!page.contains("readonly"));
        assert!(!page.contains("LOCAL SESSION"));
        assert!(page.contains("hidden"));
        assert!(!page.contains("https://"));
    }

    #[test]
    fn secrets_reject_empty_and_control_values() {
        assert!(validate_secret("", "secret").is_err());
        assert!(validate_secret("secret\n", "secret").is_err());
        assert!(validate_secret("secret", "secret").is_ok());
    }

    #[test]
    fn password_file_reader_trims_generator_newline_without_logging_contents() {
        let path = std::env::temp_dir().join(format!(
            "arqen-control-password-test-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"secret\n").unwrap();
        assert_eq!(read_password_file(&path).unwrap(), "secret");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn login_form_rejects_duplicate_credentials() {
        assert!(parse_login_form(b"username=arqen&username=other&password=secret").is_none());
        assert_eq!(
            parse_login_form(b"username=arqen&password=secret"),
            Some(("arqen".into(), "secret".into()))
        );
    }

    #[tokio::test]
    async fn successful_login_sets_session_without_www_authenticate() {
        let state = state("secret");
        let response = login(State(state), request("username=arqen&password=secret")).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(response.headers().get(header::SET_COOKIE).is_some());
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }

    #[tokio::test]
    async fn failed_login_renders_a_safe_error_without_a_session() {
        let state = state("secret");
        let response = login(State(state), request("username=arqen&password=wrong")).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Credentials not recognized."));
    }

    #[tokio::test]
    async fn proxy_requires_session_and_strips_browser_credentials() {
        let seen = Arc::new(StdMutex::new(None::<(Option<String>, bool, bool)>));
        let seen_by_upstream = Arc::clone(&seen);
        let upstream = Router::new().fallback(any(move |request: Request<Body>| {
            let seen = Arc::clone(&seen_by_upstream);
            async move {
                let auth = request
                    .headers()
                    .get(AUTH_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let has_cookie = request.headers().contains_key(header::COOKIE);
                let has_authorization = request.headers().contains_key(header::AUTHORIZATION);
                *seen.lock().unwrap() = Some((auth, has_cookie, has_authorization));
                (StatusCode::OK, "upstream")
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });

        let mut state = state("secret");
        state.ttyd_http_url = format!("http://{upstream_addr}");
        let unauthorized = proxy_http(
            State(state.clone()),
            Request::builder()
                .method(Method::GET)
                .uri("/token")
                .header(header::HOST, "127.0.0.1:7681")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert!(seen.lock().unwrap().is_none());

        let login = login(
            State(state.clone()),
            request("username=arqen&password=secret"),
        )
        .await;
        let cookie = login
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let authorized = proxy_http(
            State(state),
            Request::builder()
                .method(Method::GET)
                .uri("/token")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, cookie)
                .header(header::AUTHORIZATION, "Basic secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(authorized.status(), StatusCode::OK);
        let body = to_bytes(authorized.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"upstream");
        assert_eq!(
            seen.lock().unwrap().as_ref(),
            Some(&(Some(AUTH_VALUE.into()), false, false))
        );
    }

    #[tokio::test]
    async fn proxy_returns_bad_gateway_when_ttyd_is_unavailable() {
        let state = state("secret");
        let token = state.sessions.issue();
        let response = proxy_http(
            State({
                let mut state = state;
                state.ttyd_http_url = "http://127.0.0.1:1".into();
                state
            }),
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, format!("{SESSION_COOKIE}={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[allow(clippy::result_large_err)]
    #[tokio::test]
    async fn websocket_proxy_round_trips_binary_data_and_injects_auth_header() {
        let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        let (auth_sender, auth_receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let upstream = tokio_tungstenite::accept_hdr_async(
                stream,
                move |
                    request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                    mut response: tokio_tungstenite::tungstenite::handshake::server::Response,
                | {
                    let auth = request
                        .headers()
                        .get(AUTH_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let has_cookie = request.headers().contains_key(header::COOKIE);
                    let protocol = request
                        .headers()
                        .get(header::SEC_WEBSOCKET_PROTOCOL)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    if let Some(protocol) = request
                        .headers()
                        .get(header::SEC_WEBSOCKET_PROTOCOL)
                        .cloned()
                    {
                        response
                            .headers_mut()
                            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocol);
                    }
                    let _ = auth_sender.send((auth, has_cookie, protocol));
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let (mut sink, mut stream) = upstream.split();
            if let Some(Ok(message)) = stream.next().await {
                let _ = sink.send(message).await;
            }
        });

        let mut state = state("secret");
        state.ttyd_ws_url = format!("ws://{upstream_addr}");
        state.ttyd_host = upstream_addr.to_string();
        let token = state.sessions.issue();
        let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway_addr = gateway_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(gateway_listener, build_router(state)).await;
        });

        let mut request = format!("ws://{gateway_addr}/ws")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            header::HOST,
            HeaderValue::from_str(&gateway_addr.to_string()).unwrap(),
        );
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(&format!("http://{gateway_addr}")).unwrap(),
        );
        request.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("tty"),
        );
        let (mut client, response) = connect_async(request).await.unwrap();
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok()),
            Some("tty")
        );
        let payload = vec![0, 1, 2, 255];
        client
            .send(UpstreamMessage::Binary(payload.clone().into()))
            .await
            .unwrap();
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            UpstreamMessage::Binary(payload.into())
        );
        assert_eq!(
            auth_receiver.await.unwrap(),
            (Some(AUTH_VALUE.into()), false, Some("tty".into()))
        );
    }

    #[tokio::test]
    async fn logout_revokes_session_and_expires_cookie() {
        let state = state("secret");
        let token = state.sessions.issue();
        let response = logout(
            State(state.clone()),
            Request::builder()
                .method(Method::POST)
                .uri("/auth/logout")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, format!("{SESSION_COOKIE}={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(!state.sessions.contains(&token));
        assert!(
            response
                .headers()
                .get(header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
    }

    #[test]
    fn expired_session_is_removed_on_lookup() {
        let state = state("secret");
        let token = state.sessions.issue();
        state
            .sessions
            .sessions
            .lock()
            .unwrap()
            .insert(token.clone(), Instant::now() - Duration::from_secs(1));
        assert!(!state.sessions.contains(&token));
    }

    #[tokio::test]
    async fn cross_origin_login_is_rejected() {
        let state = state("secret");
        let mut request = request("username=arqen&password=secret");
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("http://unexpected.example"),
        );
        let response = login(State(state), request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn browser_form_with_opaque_origin_is_allowed_only_when_same_origin() {
        let same_origin_state = state("secret");
        let mut same_origin_request = request("username=arqen&password=secret");
        same_origin_request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static("null"));
        same_origin_request.headers_mut().insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        let response = login(State(same_origin_state), same_origin_request).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let cross_site_state = state("secret");
        let mut cross_site_request = request("username=arqen&password=secret");
        cross_site_request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static("null"));
        cross_site_request.headers_mut().insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("cross-site"),
        );
        let response = login(State(cross_site_state), cross_site_request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn failed_login_limiter_blocks_after_five_attempts() {
        let mut limiter = FailedLoginState::default();
        for _ in 0..MAX_FAILED_LOGINS {
            assert!(limiter.retry_after().is_none());
            limiter.record_failure();
        }
        assert!(limiter.retry_after().is_some());
    }

    #[test]
    fn websocket_messages_round_trip_binary_data() {
        let original = AxumMessage::binary(vec![0, 1, 2, 255]);
        let converted = from_upstream_message(to_upstream_message(original.clone())).unwrap();
        assert_eq!(converted, original);
    }
}

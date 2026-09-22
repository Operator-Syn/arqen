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

const LOGIN_PAGE_TEMPLATE: &str = include_str!("login.html");

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

include!("runtime.rs");
include!("routes.rs");
include!("websocket.rs");
include!("proxy.rs");
include!("auth.rs");
include!("pages.rs");
include!("tests.rs");

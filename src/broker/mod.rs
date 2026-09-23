use crate::{
    Account, AccountStore, ConnectionState,
    auth::{
        check_google_refresh_token, is_invalid_grant, is_missing_refresh_token,
        refresh_google_access_token,
    },
    gmail::{EmailListResponse, GmailApi, GmailApiError, is_read_email_too_large, is_unauthorized},
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

include!("runtime.rs");
include!("protocol.rs");
include!("handlers.rs");
include!("client.rs");
include!("tests.rs");

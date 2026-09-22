use anyhow::{Context, Result};
use arqen::{
    Account, AccountStore, ConnectionState, GMAIL_READONLY_SCOPE,
    auth::{GoogleOAuth, parse_callback, revoke_google_account, token_key},
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::backend::CrosstermBackend;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Stdio},
    time::{Duration, Instant},
};

use crate::config::{
    client_secret_path, oauth_callback_bind_addr, oauth_callback_port, oauth_callback_public_host,
    oauth_remote_mode,
};
use crate::{callback, ui};

include!("state.rs");
include!("app/mod.rs");
include!("browser.rs");
include!("input.rs");
include!("runtime.rs");
include!("tests.rs");

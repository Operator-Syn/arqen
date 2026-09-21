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

mod callback;
mod server;

const APP_DATA_DIRECTORY: &str = "arqen";
const LEGACY_DATA_DIRECTORY: &str = "google-account-tui";
// Use an Arqen-owned browser process by default so the TUI can terminate the
// isolated login window after the callback. Set this to true to use the local
// user-gesture launcher and its script-created popup instead.
pub(crate) const LOGIN_HELPER_ENABLED: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoginIntent {
    Add,
    Reauthenticate { subject: String },
    Reconnect { subject: String },
}

struct OwnedBrowser {
    child: Child,
    profile_dir: PathBuf,
    #[cfg(unix)]
    process_group: libc::pid_t,
}

impl Drop for OwnedBrowser {
    fn drop(&mut self) {
        self.terminate();
        let _ = fs::remove_dir_all(&self.profile_dir);
    }
}

impl OwnedBrowser {
    #[cfg(unix)]
    fn terminate(&mut self) {
        let process_group = -self.process_group;
        // The browser may daemonize its visible window away from the direct
        // child, so terminate the isolated process group rather than only the
        // launcher PID. The group was created exclusively for this session.
        unsafe {
            libc::kill(process_group, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unsafe {
            libc::kill(process_group, libc::SIGKILL);
        }
        let _ = self.child.wait();
    }

    #[cfg(not(unix))]
    fn terminate(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneFocus {
    Accounts,
    Details,
}

fn database_path() -> Result<std::path::PathBuf> {
    let base = env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .context("set HOME or XDG_DATA_HOME to choose the data directory")?;
    let directory = base.join(APP_DATA_DIRECTORY);
    let path = directory.join("accounts.sqlite3");
    if path.is_file() {
        return Ok(path);
    }

    let legacy_directory = base.join(LEGACY_DATA_DIRECTORY);
    let legacy_path = legacy_directory.join("accounts.sqlite3");
    if legacy_path.is_file() {
        let legacy_wal = legacy_path.with_extension("sqlite3-wal");
        let legacy_shm = legacy_path.with_extension("sqlite3-shm");
        if !legacy_wal.exists() && !legacy_shm.exists() {
            fs::create_dir_all(&directory)?;
            fs::copy(&legacy_path, &path).with_context(|| {
                format!(
                    "migrate legacy account database from {} to {}",
                    legacy_path.display(),
                    path.display()
                )
            })?;
            return Ok(path);
        }
        return Ok(legacy_path);
    }

    fs::create_dir_all(&directory)?;
    Ok(path)
}

pub(crate) fn config_directory() -> Result<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("set HOME or XDG_CONFIG_HOME to choose the config directory")?;
    Ok(base.join(APP_DATA_DIRECTORY))
}

fn client_secret_path() -> Result<std::path::PathBuf> {
    if let Some(path) = env::var_os("GOOGLE_CLIENT_SECRET") {
        return Ok(std::path::PathBuf::from(path));
    }
    let path = config_directory()?.join("google-client-secret.json");
    if path.is_file() {
        return Ok(path);
    }
    // Keep the repository-local path as a development-only compatibility
    // fallback. Installed services and direct binaries use the XDG path above.
    let path = std::path::PathBuf::from(".secrets/google-client-secret.json");
    anyhow::ensure!(
        path.is_file(),
        "Google OAuth client JSON not found; place it at {} or set GOOGLE_CLIENT_SECRET",
        config_directory()?
            .join("google-client-secret.json")
            .display()
    );
    Ok(path)
}

fn oauth_remote_mode() -> bool {
    matches!(
        env::var("ARQEN_OAUTH_REMOTE").ok().as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn oauth_callback_port() -> Result<Option<u16>> {
    if !oauth_remote_mode() {
        return Ok(None);
    }
    let raw = env::var("ARQEN_OAUTH_CALLBACK_PORT").unwrap_or_else(|_| "8765".into());
    let port = raw
        .parse::<u16>()
        .with_context(|| format!("parse ARQEN_OAUTH_CALLBACK_PORT `{raw}`"))?;
    anyhow::ensure!(port != 0, "ARQEN_OAUTH_CALLBACK_PORT cannot be zero");
    Ok(Some(port))
}

fn oauth_callback_bind_addr() -> String {
    env::var("ARQEN_OAUTH_CALLBACK_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".into())
}

fn oauth_callback_public_host() -> String {
    env::var("ARQEN_OAUTH_CALLBACK_PUBLIC_HOST").unwrap_or_else(|_| "127.0.0.1".into())
}

pub(crate) enum Screen {
    Accounts,
    Authorization {
        oauth: Option<GoogleOAuth>,
        url: String,
        callback: Option<callback::CallbackServer>,
        remote: bool,
        intent: LoginIntent,
    },
    Redirect {
        oauth: Option<GoogleOAuth>,
        input: String,
        intent: LoginIntent,
    },
    Error(String),
    ConfirmQuit,
    ConfirmDisconnect {
        subject: String,
        email: String,
        retry: bool,
    },
}

struct App {
    store: AccountStore,
    accounts: Vec<Account>,
    selected: usize,
    mcp_target_subject: Option<String>,
    pane_focus: PaneFocus,
    accounts_scroll: usize,
    details_scroll: usize,
    screen: Screen,
    browser: Option<OwnedBrowser>,
    notice: Option<String>,
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new(store: AccountStore) -> Result<Self> {
        let accounts = store.list_accounts()?;
        let mcp_target_subject = store.mcp_configuration()?.target_google_subject;
        Ok(Self {
            store,
            accounts,
            selected: 0,
            mcp_target_subject,
            pane_focus: PaneFocus::Accounts,
            accounts_scroll: 0,
            details_scroll: 0,
            screen: Screen::Accounts,
            browser: None,
            notice: None,
            clipboard: None,
        })
    }

    fn show_error(&mut self, context: &str, error: impl std::fmt::Display) {
        self.stop_browser();
        self.notice = None;
        self.screen = Screen::Error(format!("{context}\n\n{error}"));
    }

    fn stop_browser(&mut self) {
        self.browser.take();
    }

    fn reload_accounts(&mut self) -> Result<()> {
        self.accounts = self.store.list_accounts()?;
        self.mcp_target_subject = self.store.mcp_configuration()?.target_google_subject;
        self.selected = self.selected.min(self.accounts.len().saturating_sub(1));
        self.details_scroll = 0;
        Ok(())
    }

    fn select_account(&mut self, selected: usize) {
        let selected = selected.min(self.accounts.len().saturating_sub(1));
        if selected != self.selected {
            self.details_scroll = 0;
        }
        self.selected = selected;
    }

    fn toggle_mcp_target(&mut self) {
        let Some(account) = self.accounts.get(self.selected) else {
            return;
        };
        let subject = account.subject.clone();
        let email = account.email.clone();
        if self.mcp_target_subject.as_deref() == Some(subject.as_str()) {
            match self.store.set_mcp_target_subject(None) {
                Ok(()) => {
                    self.mcp_target_subject = None;
                    self.notice = Some("MCP target cleared.".into());
                }
                Err(error) => self.show_error("Could not clear MCP target", error),
            }
            return;
        }
        let Some(reason) = mcp_target_ineligibility(account) else {
            match self.store.set_mcp_target_subject(Some(&subject)) {
                Ok(()) => {
                    self.mcp_target_subject = Some(subject);
                    self.notice = Some(format!("MCP target set to {email}."));
                }
                Err(error) => self.show_error("Could not set MCP target", error),
            }
            return;
        };
        self.notice = Some(format!("Cannot set MCP target: {reason}"));
    }

    fn viewport_rows(&self) -> usize {
        let Ok((width, height)) = crossterm::terminal::size() else {
            return 5;
        };
        ui::pane_viewport(
            ratatui::layout::Rect::new(0, 0, width, height),
            self.accounts.len(),
            self.accounts
                .get(self.selected)
                .map(|account| account.connection_state),
            self.pane_focus,
        )
    }

    fn move_selection(&mut self, delta: isize) {
        if self.accounts.is_empty() {
            return;
        }
        let selected = if delta.is_negative() {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            self.selected
                .saturating_add(delta as usize)
                .min(self.accounts.len().saturating_sub(1))
        };
        self.select_account(selected);
    }

    fn scroll_details(&mut self, delta: isize) {
        if delta.is_negative() {
            self.details_scroll = self.details_scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.details_scroll = self.details_scroll.saturating_add(delta as usize);
        }
    }

    fn move_page(&mut self, forward: bool) {
        let amount = self.viewport_rows().max(1);
        match self.pane_focus {
            PaneFocus::Accounts => self.move_selection(if forward {
                amount as isize
            } else {
                -(amount as isize)
            }),
            PaneFocus::Details => self.scroll_details(if forward {
                amount as isize
            } else {
                -(amount as isize)
            }),
        }
    }

    fn move_home(&mut self) {
        match self.pane_focus {
            PaneFocus::Accounts => self.select_account(0),
            PaneFocus::Details => self.details_scroll = 0,
        }
    }

    fn move_end(&mut self) {
        match self.pane_focus {
            PaneFocus::Accounts => {
                self.select_account(self.accounts.len().saturating_sub(1));
            }
            PaneFocus::Details => self.details_scroll = usize::MAX,
        }
    }

    fn start_login(&mut self, intent: LoginIntent) {
        self.stop_browser();
        let result = (|| -> Result<Screen> {
            let remote = oauth_remote_mode();
            let callback = match oauth_callback_port()? {
                Some(port) => {
                    let bind_addr = oauth_callback_bind_addr();
                    let public_host = oauth_callback_public_host();
                    Some(
                        callback::CallbackServer::start_on_port_with_bind(
                            port,
                            &bind_addr,
                            &public_host,
                        )
                        .with_context(|| {
                            format!(
                                "bind remote OAuth callback on {public_host}:{port} (listener {bind_addr}); keep the callback route available"
                            )
                        })?,
                    )
                }
                None => callback::CallbackServer::start().ok(),
            };
            let mut oauth = GoogleOAuth::from_file(client_secret_path()?)
                .context("load Google OAuth client configuration")?;
            if let Some(callback) = callback.as_ref() {
                oauth.set_redirect_uri(callback.redirect_uri());
            }
            let url = oauth
                .authorization_url()
                .context("create Google authorization URL")?;
            if LOGIN_HELPER_ENABLED && let Some(callback) = callback.as_ref() {
                callback
                    .set_authorization_url(&url)
                    .context("prepare the Google login helper")?;
            }
            Ok(Screen::Authorization {
                oauth: Some(oauth),
                url,
                callback,
                remote,
                intent,
            })
        })();
        self.screen = match result {
            Ok(screen) => screen,
            Err(error) => {
                self.show_error("Unable to start login", format_args!("{error:#}"));
                return;
            }
        };
    }

    fn finish_login(&mut self, oauth: &mut GoogleOAuth, input: &str, intent: &LoginIntent) {
        let result = (|| -> Result<Account> {
            let expected_subject = match intent {
                LoginIntent::Add => None,
                LoginIntent::Reauthenticate { subject } | LoginIntent::Reconnect { subject } => {
                    Some(subject.as_str())
                }
            };
            let login = oauth
                .finish(
                    parse_callback(input).context("parse pasted Google redirect URL")?,
                    expected_subject,
                )
                .context("complete Google login")?;
            let profile = login.profile;
            let account = Account {
                id: uuid::Uuid::new_v4().to_string(),
                subject: profile.sub.clone(),
                email: profile.email,
                display_name: profile.name,
                token_key: Some(token_key(&profile.sub)),
                granted_scopes: Some(login.granted_scopes),
                connection_state: ConnectionState::Connected,
            };
            self.store
                .upsert_google_account(&account)
                .context("save Google account metadata in SQLite")?;
            Ok(account)
        })();
        self.stop_browser();
        match result {
            Ok(account) => self.complete_login(account),
            Err(error) => self.show_error(
                login_error_context(intent),
                friendly_login_error(intent, &error),
            ),
        }
    }

    fn complete_login(&mut self, _account: Account) {
        match self.reload_accounts() {
            Ok(()) => {}
            Err(error) => {
                self.show_error("Account connected, but refreshing accounts failed", error);
                return;
            }
        }
        self.notice = Some("Google account connected successfully.".into());
        // This is the single success boundary for both automatic and manual
        // callback flows; replacing the screen guarantees the modal is gone
        // on the next frame.
        self.screen = Screen::Accounts;
    }

    fn confirm_disconnect(&mut self, retry: bool) {
        let Some(account) = self.accounts.get(self.selected) else {
            return;
        };
        let allowed = if retry {
            account.connection_state == ConnectionState::Indeterminate
        } else {
            account.connection_state == ConnectionState::Connected
        };
        if !allowed {
            return;
        }
        self.notice = None;
        self.screen = Screen::ConfirmDisconnect {
            subject: account.subject.clone(),
            email: account.email.clone(),
            retry,
        };
    }

    fn disconnect_account(&mut self, subject: &str) {
        let token_key = self
            .accounts
            .iter()
            .find(|account| account.subject == subject)
            .and_then(|account| account.token_key.clone());
        let result = (|| -> Result<()> {
            // Persist the safe state before touching provider or credential-store state.
            // If the process stops after this point, the next run will not claim
            // that the credential is usable.
            self.store
                .set_connection_state(subject, ConnectionState::Indeterminate)
                .context("mark account connection state as indeterminate")?;
            revoke_google_account(token_key.as_deref(), subject)
                .context("revoke and remove Google credentials")?;
            self.store
                .set_connection_state(subject, ConnectionState::Disconnected)
                .context("save disconnected account state")?;
            Ok(())
        })();
        match result {
            Ok(()) => match self.reload_accounts() {
                Ok(()) => {
                    self.notice = Some("Google account disconnected successfully.".into());
                    self.screen = Screen::Accounts;
                }
                Err(error) => self.show_error(
                    "Account disconnected, but refreshing accounts failed",
                    error,
                ),
            },
            Err(error) => {
                let refresh_error = self.reload_accounts().err();
                if let Some(refresh_error) = refresh_error {
                    self.show_error(
                        "Unable to disconnect account and refresh its state",
                        format_args!("{error:#}\n\n{refresh_error:#}"),
                    );
                } else {
                    self.show_error("Unable to disconnect account", format_args!("{error:#}"));
                }
            }
        }
    }

    fn poll_callback(&mut self) {
        let target = match &mut self.screen {
            Screen::Authorization {
                callback: Some(callback),
                ..
            } => match callback.try_receive() {
                Ok(target) => target,
                Err(error) => {
                    self.show_error("OAuth callback failed", error);
                    return;
                }
            },
            _ => None,
        };
        let Some(target) = target else { return };
        let screen = std::mem::replace(&mut self.screen, Screen::Accounts);
        let (mut oauth, redirect_uri, callback, intent) = match screen {
            Screen::Authorization {
                mut oauth,
                callback: Some(callback),
                intent,
                ..
            } => (
                oauth.take(),
                callback.redirect_uri().to_owned(),
                callback,
                intent,
            ),
            _ => return,
        };
        drop(callback);
        let input = format!("{redirect_uri}{target}");
        let Some(mut oauth) = oauth.take() else {
            self.show_error("Unable to finish login", "authorization state is missing");
            return;
        };
        self.finish_login(&mut oauth, &input, &intent);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.screen {
            Screen::Accounts => match key.code {
                KeyCode::Tab => {
                    self.pane_focus = match self.pane_focus {
                        PaneFocus::Accounts => PaneFocus::Details,
                        PaneFocus::Details => PaneFocus::Accounts,
                    };
                }
                KeyCode::BackTab => {
                    self.pane_focus = match self.pane_focus {
                        PaneFocus::Accounts => PaneFocus::Details,
                        PaneFocus::Details => PaneFocus::Accounts,
                    };
                }
                KeyCode::Char('a') => self.start_login(LoginIntent::Add),
                KeyCode::Char('t') => self.toggle_mcp_target(),
                KeyCode::Char('d') => {
                    let retry = self.accounts.get(self.selected).is_some_and(|account| {
                        account.connection_state == ConnectionState::Indeterminate
                    });
                    self.confirm_disconnect(retry);
                }
                KeyCode::Char('l') => {
                    let subject = self
                        .accounts
                        .get(self.selected)
                        .filter(|account| {
                            matches!(
                                account.connection_state,
                                ConnectionState::Disconnected | ConnectionState::Indeterminate
                            )
                        })
                        .map(|account| account.subject.clone());
                    if let Some(subject) = subject {
                        self.start_login(LoginIntent::Reconnect { subject });
                    }
                }
                KeyCode::Char('r') => {
                    let subject = self
                        .accounts
                        .get(self.selected)
                        .map(|account| account.subject.clone());
                    if let Some(subject) = subject {
                        self.start_login(LoginIntent::Reauthenticate { subject });
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => match self.pane_focus {
                    PaneFocus::Accounts => self.move_selection(-1),
                    PaneFocus::Details => self.scroll_details(-1),
                },
                KeyCode::Down | KeyCode::Char('j') => match self.pane_focus {
                    PaneFocus::Accounts => self.move_selection(1),
                    PaneFocus::Details => self.scroll_details(1),
                },
                KeyCode::PageUp => self.move_page(false),
                KeyCode::PageDown => self.move_page(true),
                KeyCode::Home => self.move_home(),
                KeyCode::End => self.move_end(),
                KeyCode::Enter => {
                    if let Some(account) = self.accounts.get(self.selected) {
                        self.notice = Some(format!(
                            "Selected {} ({})",
                            account.display_name.as_deref().unwrap_or("Unnamed account"),
                            account.email
                        ));
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {}
                _ => {}
            },
            Screen::Authorization {
                oauth,
                url,
                callback,
                remote,
                intent,
            } => match key.code {
                KeyCode::Char('c') => {
                    let result = copy_to_clipboard(&mut self.clipboard, url);
                    self.notice = Some(match result {
                        Ok(()) => "Authorization URL copied to clipboard.".into(),
                        Err(error) => {
                            self.show_error(
                                "Could not copy authorization URL",
                                format_args!("{error:#}"),
                            );
                            return;
                        }
                    });
                }
                KeyCode::Char('o') => {
                    if *remote {
                        self.notice = Some(
                            "Remote OAuth mode is active. Open the authorization URL in your local browser while the SSH tunnel remains connected.".into(),
                        );
                        return;
                    }
                    let using_helper = LOGIN_HELPER_ENABLED && callback.is_some();
                    let browser_url = browser_target(callback.as_ref(), url);
                    self.browser.take();
                    let result = launch_browser(&browser_url);
                    match result {
                        Ok(browser) => {
                            let owned = browser.is_some();
                            self.browser = browser;
                            self.notice = Some(if using_helper {
                                "Login helper opened in your browser; click Continue to Google there."
                                    .into()
                            } else if owned {
                                "Dedicated Google login window opened; Arqen will close it after login completes."
                                    .into()
                            } else {
                                "Google sign-in opened in your browser; complete sign-in there, then return to Arqen."
                                    .into()
                            });
                        }
                        Err(error) => {
                            self.show_error(
                                if using_helper {
                                    "Could not open login helper"
                                } else {
                                    "Could not open authorization URL"
                                },
                                format_args!("{error:#}"),
                            );
                        }
                    }
                }
                KeyCode::Enter => {
                    if callback.is_some() {
                        self.notice = Some(
                            if *remote {
                                "Open the URL in your local browser; the SSH tunnel will deliver the callback to this TUI."
                            } else if LOGIN_HELPER_ENABLED {
                                "Press [o] to open the login helper."
                            } else {
                                "Press [o] to open Google sign-in, or [c] to copy the URL."
                            }
                            .into(),
                        );
                        return;
                    }
                    let Some(oauth) = oauth.take() else {
                        self.show_error(
                            "Unable to continue login",
                            "authorization state is missing",
                        );
                        return;
                    };
                    let url = std::mem::take(url);
                    self.screen = Screen::Redirect {
                        oauth: Some(oauth),
                        input: String::new(),
                        intent: intent.clone(),
                    };
                    self.notice = Some(format!("Open this URL in your browser:\n{url}"));
                }
                KeyCode::Esc => {
                    self.stop_browser();
                    self.screen = Screen::Accounts;
                }
                _ => {}
            },
            Screen::Redirect {
                oauth,
                input,
                intent,
            } => match key.code {
                KeyCode::Enter => {
                    let input = std::mem::take(input);
                    let login_intent = intent.clone();
                    let Some(mut oauth) = oauth.take() else {
                        self.show_error("Unable to finish login", "authorization state is missing");
                        return;
                    };
                    self.finish_login(&mut oauth, &input, &login_intent);
                }
                KeyCode::Esc => {
                    self.stop_browser();
                    self.screen = Screen::Accounts;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.push(ch)
                }
                _ => {}
            },
            Screen::Error(_) => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    self.screen = Screen::Accounts;
                }
            }
            Screen::ConfirmDisconnect { subject, .. } => match key.code {
                KeyCode::Enter | KeyCode::Char('y') => {
                    let subject = subject.clone();
                    self.disconnect_account(&subject);
                }
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.screen = Screen::Accounts;
                    self.notice = Some("Disconnect cancelled.".into());
                }
                _ => {}
            },
            Screen::ConfirmQuit => {}
        }
    }
}

fn browser_target(callback: Option<&callback::CallbackServer>, authorization_url: &str) -> String {
    browser_target_with_mode(LOGIN_HELPER_ENABLED, callback, authorization_url)
}

fn browser_target_with_mode(
    login_helper_enabled: bool,
    callback: Option<&callback::CallbackServer>,
    authorization_url: &str,
) -> String {
    if login_helper_enabled {
        callback
            .map(|callback| callback.launcher_uri().to_owned())
            .unwrap_or_else(|| authorization_url.to_owned())
    } else {
        authorization_url.to_owned()
    }
}

fn login_error_context(intent: &LoginIntent) -> &'static str {
    match intent {
        LoginIntent::Add => "Login failed",
        LoginIntent::Reauthenticate { .. } => "Reauthentication not completed",
        LoginIntent::Reconnect { .. } => "Reconnection not completed",
    }
}

fn mcp_target_ineligibility(account: &Account) -> Option<&'static str> {
    if account.connection_state != ConnectionState::Connected {
        return Some("the account is not connected");
    }
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return Some("Google's granted scopes are unverified");
    };
    if !scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE) {
        return Some("the recorded grant does not include Gmail read-only access");
    }
    if account.token_key.is_none() {
        return Some("the account has no protected credential reference");
    }
    None
}

fn friendly_login_error(intent: &LoginIntent, error: &anyhow::Error) -> String {
    let detail = format!("{error:#}");
    if !detail.contains(arqen::auth::SUBJECT_MISMATCH_MESSAGE) {
        return detail;
    }
    match intent {
        LoginIntent::Reauthenticate { .. } => {
            "Google returned a different account than the one selected for reauthentication.\n\nNo account data or credentials were changed. Close this message and sign in with the same Google account to try again.".into()
        }
        LoginIntent::Reconnect { .. } => {
            "Google returned a different account than the one on this card.\n\nNo account data or credentials were changed. Close this message and sign in with the card's Google account to reconnect it.".into()
        }
        LoginIntent::Add => detail,
    }
}

fn copy_to_clipboard(clipboard: &mut Option<arboard::Clipboard>, text: &str) -> Result<()> {
    if clipboard.is_none() {
        match arboard::Clipboard::new() {
            Ok(value) => *clipboard = Some(value),
            Err(error) => return copy_with_fallback(text, anyhow::anyhow!(error)),
        }
    }
    if let Some(value) = clipboard.as_mut() {
        match value
            .set_text(text)
            .context("write authorization URL to clipboard")
        {
            Ok(()) => return Ok(()),
            Err(error) => {
                *clipboard = None;
                return copy_with_fallback(text, error);
            }
        }
    }
    anyhow::bail!("clipboard provider was unavailable")
}

fn copy_with_fallback(text: &str, primary_error: anyhow::Error) -> Result<()> {
    let mut failures = Vec::new();
    for (program, args) in [
        ("wl-copy", Vec::new()),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ] {
        let mut child = match std::process::Command::new(program)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                failures.push(format!("{program}: {error}"));
                continue;
            }
        };
        let write_result = child
            .stdin
            .take()
            .context("open clipboard fallback input")
            .and_then(|mut input| {
                input
                    .write_all(text.as_bytes())
                    .context("write clipboard fallback input")
            });
        if let Err(error) = write_result {
            failures.push(format!("{program}: {error}"));
            let _ = child.kill();
            continue;
        }
        if let Some(status) = child.try_wait().context("check clipboard fallback")? {
            if status.success() {
                return Ok(());
            }
            failures.push(format!("{program}: exited with {status}"));
        } else {
            return Ok(());
        }
    }
    anyhow::bail!(
        "{primary_error:#}; clipboard fallbacks unavailable ({})",
        failures.join("; ")
    )
}

fn open_in_browser(url: &str) -> Result<()> {
    let candidates = browser_candidates(url, env::var_os("BROWSER"));
    let mut failures = Vec::new();
    for (program, args) in candidates {
        let result = std::process::Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(_) => return Ok(()),
            Err(error) => failures.push(format!("{}: {error}", program.to_string_lossy())),
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "no browser launcher was available ({})",
            failures.join("; ")
        )
    } else {
        anyhow::bail!("no browser launcher was configured")
    }
}

fn launch_browser(url: &str) -> Result<Option<OwnedBrowser>> {
    // A unique profile is what makes terminating the child safe: Chromium and
    // Firefox cannot route this login into the user's normal browser process.
    let profile_dir = match create_browser_profile() {
        Ok(path) => path,
        Err(profile_error) => {
            open_in_browser(url).with_context(|| {
                format!(
                    "create an isolated browser profile ({profile_error:#}); browser fallback also failed"
                )
            })?;
            return Ok(None);
        }
    };
    let mut failures = Vec::new();
    for (program, _) in browser_candidates(url, env::var_os("BROWSER")) {
        let Some(args) = owned_browser_arguments(&program, url, &profile_dir) else {
            continue;
        };
        let mut command = std::process::Command::new(&program);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let result = command.spawn();
        match result {
            Ok(child) => {
                #[cfg(unix)]
                let process_group = child.id() as libc::pid_t;
                return Ok(Some(OwnedBrowser {
                    child,
                    profile_dir,
                    #[cfg(unix)]
                    process_group,
                }));
            }
            Err(error) => failures.push(format!("{}: {error}", program.to_string_lossy())),
        }
    }

    let _ = fs::remove_dir_all(&profile_dir);
    open_in_browser(url).with_context(|| {
        if failures.is_empty() {
            "no isolated browser launcher was available; browser fallback also failed".to_owned()
        } else {
            format!(
                "isolated browser launchers were unavailable ({}); browser fallback also failed",
                failures.join("; ")
            )
        }
    })?;
    Ok(None)
}

fn create_browser_profile() -> Result<PathBuf> {
    let base = env::temp_dir();
    for _ in 0..8 {
        let path = base.join(format!("arqen-oauth-{}", uuid::Uuid::new_v4().simple()));
        match fs::create_dir(&path) {
            Ok(()) => {
                if let Err(error) = secure_browser_profile(&path) {
                    let _ = fs::remove_dir_all(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("create temporary browser profile at {}", path.display())
                });
            }
        }
    }
    anyhow::bail!("could not allocate a unique temporary browser profile")
}

fn secure_browser_profile(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("restrict temporary browser profile at {}", path.display()))?;
    }
    Ok(())
}

fn browser_candidates(
    url: &str,
    configured_browser: Option<std::ffi::OsString>,
) -> Vec<(std::ffi::OsString, Vec<std::ffi::OsString>)> {
    let mut candidates: Vec<(std::ffi::OsString, Vec<std::ffi::OsString>)> = Vec::new();
    // Prefer the explicitly configured browser. Known browser launchers get a
    // new-window request so the external link is visible and active instead of
    // being silently forwarded to a background tab.
    if let Some(browser) = configured_browser {
        candidates.push((browser.clone(), browser_arguments(&browser, url)));
    }
    // Direct browser fallbacks also request a new window so the browser can
    // activate its window. Desktop URL handlers are retained below as
    // portable fallbacks, but may hand the URL to an existing browser without
    // bringing it to the front.
    for browser in ["brave", "google-chrome-stable", "google-chrome", "chromium"] {
        candidates.push((browser.into(), vec!["--new-window".into(), url.into()]));
    }
    candidates.push(("firefox".into(), vec!["--new-window".into(), url.into()]));
    candidates.push(("gio".into(), vec!["open".into(), url.into()]));
    candidates.push(("xdg-open".into(), vec![url.into()]));
    candidates
}

fn browser_arguments(program: &std::ffi::OsStr, url: &str) -> Vec<std::ffi::OsString> {
    if browser_kind(program).is_some() {
        vec!["--new-window".into(), url.into()]
    } else {
        vec![url.into()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserKind {
    Chromium,
    Firefox,
}

fn browser_kind(program: &OsStr) -> Option<BrowserKind> {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.contains("firefox") {
        Some(BrowserKind::Firefox)
    } else if name.contains("brave") || name.contains("chrome") || name.contains("chromium") {
        Some(BrowserKind::Chromium)
    } else {
        None
    }
}

fn owned_browser_arguments(
    program: &OsStr,
    url: &str,
    profile_dir: &Path,
) -> Option<Vec<OsString>> {
    match browser_kind(program)? {
        BrowserKind::Chromium => {
            let mut profile = OsString::from("--user-data-dir=");
            profile.push(profile_dir.as_os_str());
            Some(vec![
                profile,
                "--no-first-run".into(),
                "--no-default-browser-check".into(),
                "--disable-sync".into(),
                "--new-window".into(),
                url.into(),
            ])
        }
        BrowserKind::Firefox => Some(vec![
            "--no-remote".into(),
            "--profile".into(),
            profile_dir.as_os_str().to_owned(),
            "--new-window".into(),
            url.into(),
        ]),
    }
}

fn handle_mouse(app: &mut App, column: u16, row: u16) -> bool {
    let Ok((width, height)) = crossterm::terminal::size() else {
        return false;
    };
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    if !matches!(app.screen, Screen::Accounts) {
        if let Some(action) =
            ui::modal_action(area, &app.screen, column, row, app.notice.as_deref())
        {
            use ui::modal::ModalActionId;
            if matches!(action, ModalActionId::Confirm) {
                match &app.screen {
                    Screen::ConfirmQuit => return true,
                    Screen::ConfirmDisconnect { subject, .. } => {
                        let subject = subject.clone();
                        app.disconnect_account(&subject);
                    }
                    _ => {}
                }
                return false;
            }
            if matches!(app.screen, Screen::ConfirmQuit) && matches!(action, ModalActionId::Cancel)
            {
                app.screen = Screen::Accounts;
                app.notice = Some("Quit cancelled.".into());
                return false;
            }
            let key = match action {
                ModalActionId::Copy => KeyCode::Char('c'),
                ModalActionId::Open => KeyCode::Char('o'),
                ModalActionId::Continue => KeyCode::Enter,
                ModalActionId::Cancel | ModalActionId::Close => KeyCode::Esc,
                ModalActionId::Confirm => unreachable!(),
            };
            app.handle_key(KeyEvent::new(key, KeyModifiers::NONE));
        }
        return false;
    }

    if let Screen::Accounts = &app.screen {
        if let Some(focus) = ui::pane_at_with_notice(
            area,
            &app.accounts,
            app.selected,
            column,
            row,
            app.notice.as_deref(),
        ) {
            app.pane_focus = focus;
        }
        match ui::mouse_target_with_focus(
            area,
            &app.accounts,
            app.selected,
            column,
            row,
            ui::InteractionContext {
                pane_focus: app.pane_focus,
                accounts_scroll: app.accounts_scroll,
            },
            app.notice.as_deref(),
        ) {
            Some(ui::MouseTarget::Account(index)) => {
                app.pane_focus = PaneFocus::Accounts;
                app.select_account(index);
                app.notice = Some(format!(
                    "Selected {}",
                    app.accounts[index]
                        .display_name
                        .as_deref()
                        .unwrap_or("Unnamed account")
                ));
            }
            Some(ui::MouseTarget::AddAccount) => app.start_login(LoginIntent::Add),
            Some(ui::MouseTarget::Reauthenticate) => {
                let subject = app
                    .accounts
                    .get(app.selected)
                    .map(|account| account.subject.clone());
                if let Some(subject) = subject {
                    app.start_login(LoginIntent::Reauthenticate { subject });
                }
            }
            Some(ui::MouseTarget::Disconnect) => {
                let retry = app.accounts.get(app.selected).is_some_and(|account| {
                    account.connection_state == ConnectionState::Indeterminate
                });
                app.confirm_disconnect(retry);
            }
            Some(ui::MouseTarget::Login) => {
                let subject = app
                    .accounts
                    .get(app.selected)
                    .filter(|account| {
                        matches!(
                            account.connection_state,
                            ConnectionState::Disconnected | ConnectionState::Indeterminate
                        )
                    })
                    .map(|account| account.subject.clone());
                if let Some(subject) = subject {
                    app.start_login(LoginIntent::Reconnect { subject });
                }
            }
            Some(ui::MouseTarget::ConnectionBadge) => {
                app.pane_focus = PaneFocus::Details;
                let state = app
                    .accounts
                    .get(app.selected)
                    .map(|account| account.connection_state);
                match state {
                    Some(ConnectionState::Connected) => app.confirm_disconnect(false),
                    Some(ConnectionState::Indeterminate) => app.confirm_disconnect(true),
                    Some(ConnectionState::Disconnected) => {
                        if let Some(subject) = app
                            .accounts
                            .get(app.selected)
                            .map(|account| account.subject.clone())
                        {
                            app.start_login(LoginIntent::Reconnect { subject });
                        }
                    }
                    None => {}
                }
            }
            Some(ui::MouseTarget::Focus) => {
                app.pane_focus = match app.pane_focus {
                    PaneFocus::Accounts => PaneFocus::Details,
                    PaneFocus::Details => PaneFocus::Accounts,
                };
            }
            None => {}
        }
    }
    false
}

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("credential-broker") => return run_credential_broker(),
        Some("mcp-server") => return run_mcp_server(),
        Some("--help") | Some("-h") => {
            print_help();
            return Ok(());
        }
        Some(command) => anyhow::bail!("unknown Arqen command `{command}`; use --help"),
        None => {}
    }
    let path = database_path().with_context(|| "open application data directory")?;
    let store = AccountStore::open(&path)
        .with_context(|| format!("open account database at {}", path.display()))?;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let result = run_tui(&mut stdout, App::new(store)?);
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    result
}

fn run_credential_broker() -> Result<()> {
    let socket_path = configured_broker_socket()?;
    let database_path = database_path().context("open application data directory")?;
    let credentials_path = client_secret_path()?;
    arqen::broker::run(arqen::broker::BrokerOptions {
        socket_path,
        database_path,
        credentials_path,
    })
}

fn run_mcp_server() -> Result<()> {
    let options = server::ServerOptions::from_env(configured_broker_socket()?)?;
    server::run(options)
}

fn configured_broker_socket() -> Result<PathBuf> {
    match env::var_os("ARQEN_GMAIL_BROKER_SOCKET") {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        Some(_) => anyhow::bail!("ARQEN_GMAIL_BROKER_SOCKET cannot be empty"),
        None => arqen::broker::default_socket_path(),
    }
}

fn print_help() {
    println!(
        "Arqen\n\nCommands:\n  credential-broker  Serve protected-store-backed Gmail access over a Unix socket\n  mcp-server         Serve the Streamable HTTP MCP endpoint\n\nWith no command, start the interactive account TUI."
    );
}

fn run_tui(stdout: &mut io::Stdout, mut app: App) -> Result<()> {
    let backend = CrosstermBackend::new(&mut *stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.hide_cursor()?;
    loop {
        app.poll_callback();
        terminal.draw(|frame| {
            ui::draw(
                frame,
                &app.accounts,
                app.selected,
                app.mcp_target_subject.as_deref(),
                app.pane_focus,
                &mut app.accounts_scroll,
                &mut app.details_scroll,
                &app.screen,
                app.notice.as_deref(),
            )
        })?;
        if event::poll(std::time::Duration::from_millis(250))? {
            let should_quit = handle_event(event::read()?, &mut app);
            if should_quit {
                break;
            }
        }
    }
    terminal.show_cursor()?;
    Ok(())
}

fn handle_event(event: Event, app: &mut App) -> bool {
    match event {
        Event::Key(key) => {
            if key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
                && !matches!(
                    app.screen,
                    Screen::ConfirmQuit | Screen::ConfirmDisconnect { .. }
                )
            {
                app.screen = Screen::ConfirmQuit;
                app.notice = None;
                return false;
            }
            match (&app.screen, key.code) {
                (Screen::Accounts, KeyCode::Char('q') | KeyCode::Esc) => {
                    app.screen = Screen::ConfirmQuit;
                    return false;
                }
                (Screen::ConfirmQuit, KeyCode::Enter | KeyCode::Char('y')) => return true,
                (Screen::ConfirmQuit, KeyCode::Esc | KeyCode::Char('n')) => {
                    app.screen = Screen::Accounts;
                    app.notice = Some("Quit cancelled.".into());
                    return false;
                }
                (Screen::ConfirmQuit, _) => return false,
                _ => {}
            }
            app.handle_key(key);
        }
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::Down(MouseButton::Left)
                if handle_mouse(app, mouse.column, mouse.row) =>
            {
                return true;
            }
            MouseEventKind::ScrollUp => handle_scroll_mouse(app, mouse.column, mouse.row, false),
            MouseEventKind::ScrollDown => handle_scroll_mouse(app, mouse.column, mouse.row, true),
            MouseEventKind::Moved => handle_mouse_move(app, mouse.column, mouse.row),
            _ => {}
        },
        _ => {}
    }
    false
}

fn handle_scroll_mouse(app: &mut App, column: u16, row: u16, forward: bool) {
    if !matches!(app.screen, Screen::Accounts) {
        return;
    }
    let Ok((width, height)) = crossterm::terminal::size() else {
        return;
    };
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    handle_scroll_mouse_at(app, area, column, row, forward);
}

fn handle_scroll_mouse_at(
    app: &mut App,
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
    forward: bool,
) {
    if !matches!(app.screen, Screen::Accounts) {
        return;
    }
    let Some(focus) = ui::pane_at_with_notice(
        area,
        &app.accounts,
        app.selected,
        column,
        row,
        app.notice.as_deref(),
    ) else {
        return;
    };
    app.pane_focus = focus;
    match focus {
        PaneFocus::Accounts => app.move_selection(if forward { 3 } else { -3 }),
        PaneFocus::Details => app.scroll_details(if forward { 3 } else { -3 }),
    }
}

fn handle_mouse_move(app: &mut App, column: u16, row: u16) {
    if !matches!(app.screen, Screen::Accounts) {
        return;
    }
    let Ok((width, height)) = crossterm::terminal::size() else {
        return;
    };
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    if let Some(focus) = ui::pane_at_with_notice(
        area,
        &app.accounts,
        app.selected,
        column,
        row,
        app.notice.as_deref(),
    ) {
        app.pane_focus = focus;
    }
}

mod ui;

#[cfg(test)]
mod tests {
    use super::{App, LoginIntent, PaneFocus, Screen, handle_event, handle_scroll_mouse_at};
    use arqen::{Account, AccountStore, ConnectionState, GMAIL_READONLY_SCOPE};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use std::{
        ffi::{OsStr, OsString},
        path::Path,
    };

    fn app_with_accounts() -> App {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "first".into(),
                subject: "subject-first".into(),
                email: "first@example.com".into(),
                display_name: Some("First Account".into()),
                token_key: Some("google/first".into()),
                granted_scopes: None,
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
        store
            .upsert_google_account(&Account {
                id: "second".into(),
                subject: "subject-second".into(),
                email: "second@example.com".into(),
                display_name: Some("Second Account".into()),
                token_key: Some("google/second".into()),
                granted_scopes: None,
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
        App::new(store).unwrap()
    }

    #[test]
    fn account_selection_is_keyboard_navigable_and_bounded() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn pane_focus_routes_navigation_and_resets_details_on_selection_change() {
        let mut app = app_with_accounts();
        assert_eq!(app.pane_focus, PaneFocus::Accounts);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.pane_focus, PaneFocus::Details);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, 1);
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert!(app.details_scroll > 1);
        app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert!(app.details_scroll < 6);
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, 0);
        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, usize::MAX);

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.pane_focus, PaneFocus::Accounts);
        app.details_scroll = 4;
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        assert_eq!(app.details_scroll, 0);

        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn mcp_target_toggle_persists_and_replaces_the_selected_account() {
        let mut app = app_with_accounts();
        for account in &app.accounts {
            app.store
                .upsert_google_account(&Account {
                    granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into()]),
                    ..account.clone()
                })
                .unwrap();
        }
        app.reload_accounts().unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject.as_deref(), Some("subject-first"));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("first@example.com"))
        );

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject.as_deref(), Some("subject-second"));
        assert_eq!(
            app.store
                .mcp_configuration()
                .unwrap()
                .target_google_subject
                .as_deref(),
            Some("subject-second")
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject, None);
    }

    #[test]
    fn mcp_target_toggle_explains_ineligible_accounts_without_changing_configuration() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject, None);
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("granted scopes are unverified"))
        );
    }

    #[test]
    fn mouse_wheel_focuses_and_scrolls_the_pane_under_the_pointer() {
        let mut app = app_with_accounts();
        let area = Rect::new(0, 0, 120, 32);

        handle_scroll_mouse_at(&mut app, area, 2, 8, true);
        assert_eq!(app.pane_focus, PaneFocus::Accounts);
        assert_eq!(app.selected, 1);

        app.selected = 0;
        app.details_scroll = 0;
        handle_scroll_mouse_at(&mut app, area, 70, 8, true);
        assert_eq!(app.pane_focus, PaneFocus::Details);
        assert_eq!(app.details_scroll, 3);
    }

    #[test]
    fn enter_reports_selected_account_and_escape_closes_error() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.notice.as_deref().unwrap().contains("First Account"));
        app.screen = Screen::Error("test error".into());
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.screen, Screen::Accounts));
    }

    #[test]
    fn disconnect_requires_confirmation_and_cancel_preserves_connection() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(matches!(
            app.screen,
            Screen::ConfirmDisconnect {
                retry: false,
                ref subject,
                ..
            } if subject == "subject-first"
        ));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.screen, Screen::Accounts));
        assert_eq!(app.accounts[0].connection_state, ConnectionState::Connected);
    }

    #[test]
    fn indeterminate_disconnect_offers_retry_confirmation() {
        let mut app = app_with_accounts();
        app.store
            .set_connection_state("subject-first", ConnectionState::Indeterminate)
            .unwrap();
        app.reload_accounts().unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(matches!(
            app.screen,
            Screen::ConfirmDisconnect { retry: true, .. }
        ));
    }

    #[test]
    fn quitting_requires_confirmation_and_can_be_cancelled() {
        let mut app = app_with_accounts();
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::Accounts));
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(handle_event(
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &mut app,
        ));
    }

    #[test]
    fn runtime_errors_are_rendered_in_the_error_screen() {
        let mut app = app_with_accounts();
        app.notice = Some("stale notice".into());
        app.show_error("Clipboard failed", "provider unavailable");
        assert!(
            matches!(app.screen, Screen::Error(ref message) if message.contains("Clipboard failed") && message.contains("provider unavailable"))
        );
        assert!(app.notice.is_none());
    }

    #[test]
    fn subject_mismatch_login_error_explains_recovery_without_internal_context() {
        let error = anyhow::anyhow!(
            "complete Google login: {}",
            arqen::auth::SUBJECT_MISMATCH_MESSAGE
        );
        let message = super::friendly_login_error(
            &LoginIntent::Reauthenticate {
                subject: "selected-subject".into(),
            },
            &error,
        );
        assert!(message.contains("different account"));
        assert!(message.contains("No account data or credentials were changed"));
        assert!(!message.contains("complete Google login"));
    }

    #[test]
    fn browser_target_defaults_to_the_direct_authorization_url() {
        let callback = crate::callback::CallbackServer::start().expect("callback server");
        let direct_url = "https://accounts.google.com/o/oauth2/v2/auth?state=test";
        let launcher_url = super::browser_target(Some(&callback), direct_url);
        assert_eq!(launcher_url, direct_url);
        assert_eq!(super::browser_target(None, direct_url), direct_url);
    }

    #[test]
    fn browser_target_can_restore_the_local_launcher_with_the_toggle() {
        let callback = crate::callback::CallbackServer::start().expect("callback server");
        let direct_url = "https://accounts.google.com/o/oauth2/v2/auth?state=test";
        let launcher_url = super::browser_target_with_mode(true, Some(&callback), direct_url);
        assert!(launcher_url.starts_with("http://127.0.0.1:"));
        assert!(launcher_url.contains("/oauth2/launch/"));
        assert_ne!(launcher_url, direct_url);
        assert_eq!(
            super::browser_target_with_mode(false, Some(&callback), direct_url),
            direct_url
        );
    }

    #[test]
    fn browser_candidates_prioritize_foreground_browser_launchers() {
        let candidates = super::browser_candidates("http://127.0.0.1:12345", None);
        let gio_index = candidates
            .iter()
            .position(|(program, _)| program == "gio")
            .expect("gio fallback");
        let brave_index = candidates
            .iter()
            .position(|(program, _)| program == "brave")
            .expect("brave launcher");
        assert!(brave_index < gio_index);
        assert_eq!(candidates[brave_index].1[0], "--new-window");
    }

    #[test]
    fn configured_browser_gets_a_new_window_when_it_is_a_browser_launcher() {
        let candidates = super::browser_candidates(
            "http://127.0.0.1:12345",
            Some("/nix/store/brave-gpu-picker/bin/brave-gpu-picker".into()),
        );
        assert_eq!(
            candidates[0].1,
            vec![
                std::ffi::OsString::from("--new-window"),
                std::ffi::OsString::from("http://127.0.0.1:12345")
            ]
        );
    }

    #[test]
    fn configured_non_browser_launcher_keeps_its_original_argument_shape() {
        let candidates =
            super::browser_candidates("http://127.0.0.1:12345", Some("custom-url-handler".into()));
        assert_eq!(
            candidates[0].1,
            vec![std::ffi::OsString::from("http://127.0.0.1:12345")]
        );
    }

    #[test]
    fn owned_chromium_arguments_use_an_isolated_profile_and_new_window() {
        let arguments = super::owned_browser_arguments(
            OsStr::new("brave-gpu-picker"),
            "http://127.0.0.1:12345",
            Path::new("/tmp/arqen-oauth-test"),
        )
        .expect("Chromium browser arguments");
        assert_eq!(
            arguments[0],
            OsString::from("--user-data-dir=/tmp/arqen-oauth-test")
        );
        assert!(arguments.contains(&OsString::from("--no-first-run")));
        assert!(arguments.contains(&OsString::from("--new-window")));
        assert_eq!(
            arguments.last(),
            Some(&OsString::from("http://127.0.0.1:12345"))
        );
    }

    #[test]
    fn owned_firefox_arguments_disable_remote_reuse() {
        let arguments = super::owned_browser_arguments(
            OsStr::new("firefox"),
            "http://127.0.0.1:12345",
            Path::new("/tmp/arqen-oauth-test"),
        )
        .expect("Firefox browser arguments");
        assert_eq!(arguments[0], OsString::from("--no-remote"));
        assert_eq!(arguments[1], OsString::from("--profile"));
        assert!(arguments.contains(&OsString::from("--new-window")));
        assert_eq!(
            arguments.last(),
            Some(&OsString::from("http://127.0.0.1:12345"))
        );
    }

    #[test]
    fn non_browser_handlers_cannot_be_owned() {
        assert!(
            super::owned_browser_arguments(
                OsStr::new("gio"),
                "http://127.0.0.1:12345",
                Path::new("/tmp/arqen-oauth-test"),
            )
            .is_none()
        );
    }

    #[test]
    fn successful_completion_replaces_any_login_screen() {
        let mut app = app_with_accounts();
        app.screen = Screen::Error("stale login modal".into());
        let account = Account {
            id: "completed".into(),
            subject: "subject-completed".into(),
            email: "completed@example.com".into(),
            display_name: Some("Completed".into()),
            token_key: Some("keyring:arqen:subject-completed".into()),
            granted_scopes: Some(vec!["openid".into()]),
            connection_state: ConnectionState::Connected,
        };
        app.store.upsert_google_account(&account).unwrap();
        app.complete_login(account);
        assert!(matches!(app.screen, Screen::Accounts));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("successfully"))
        );
        assert!(
            app.accounts
                .iter()
                .any(|account| account.email == "completed@example.com")
        );
    }

    #[test]
    fn ctrl_c_opens_quit_confirmation_from_any_screen() {
        let mut app = app_with_accounts();
        app.screen = Screen::Authorization {
            oauth: None,
            url: "https://accounts.google.com".into(),
            callback: None,
            remote: false,
            intent: LoginIntent::Add,
        };
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
    }
}

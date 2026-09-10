use anyhow::{Context, Result};
use arqen::{
    Account, AccountStore, ConnectionState,
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
use std::{env, fs, io, io::Write, process::Stdio};

mod callback;

const APP_DATA_DIRECTORY: &str = "arqen";
const LEGACY_DATA_DIRECTORY: &str = "google-account-tui";

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoginIntent {
    Add,
    Reauthenticate { subject: String },
    Reconnect { subject: String },
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

fn client_secret_path() -> Result<std::path::PathBuf> {
    if let Some(path) = env::var_os("GOOGLE_CLIENT_SECRET") {
        return Ok(std::path::PathBuf::from(path));
    }
    let path = std::path::PathBuf::from(".secrets/google-client-secret.json");
    anyhow::ensure!(
        path.is_file(),
        "Google OAuth client JSON not found; place it at {} or set GOOGLE_CLIENT_SECRET",
        path.display()
    );
    Ok(path)
}

pub(crate) enum Screen {
    Accounts,
    Authorization {
        oauth: Option<GoogleOAuth>,
        url: String,
        callback: Option<callback::CallbackServer>,
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
    pane_focus: PaneFocus,
    accounts_scroll: usize,
    details_scroll: usize,
    screen: Screen,
    notice: Option<String>,
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new(store: AccountStore) -> Result<Self> {
        let accounts = store.list_accounts()?;
        Ok(Self {
            store,
            accounts,
            selected: 0,
            pane_focus: PaneFocus::Accounts,
            accounts_scroll: 0,
            details_scroll: 0,
            screen: Screen::Accounts,
            notice: None,
            clipboard: None,
        })
    }

    fn show_error(&mut self, context: &str, error: impl std::fmt::Display) {
        self.notice = None;
        self.screen = Screen::Error(format!("{context}\n\n{error}"));
    }

    fn reload_accounts(&mut self) -> Result<()> {
        self.accounts = self.store.list_accounts()?;
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
        let result = (|| -> Result<Screen> {
            let callback = callback::CallbackServer::start().ok();
            let mut oauth = GoogleOAuth::from_file(client_secret_path()?)
                .context("load Google OAuth client configuration")?;
            if let Some(callback) = callback.as_ref() {
                oauth.set_redirect_uri(callback.redirect_uri());
            }
            let url = oauth
                .authorization_url()
                .context("create Google authorization URL")?;
            Ok(Screen::Authorization {
                oauth: Some(oauth),
                url,
                callback,
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
            // Persist the safe state before touching provider or keyring state.
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
                    let result = open_in_browser(url);
                    self.notice = Some(match result {
                        Ok(()) => "Browser opened; switch to it if it did not come forward.".into(),
                        Err(error) => {
                            self.show_error(
                                "Could not open authorization URL",
                                format_args!("{error:#}"),
                            );
                            return;
                        }
                    });
                }
                KeyCode::Enter => {
                    if callback.is_some() {
                        self.notice = Some("Press [o] to open the authorization URL.".into());
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
                KeyCode::Esc => self.screen = Screen::Accounts,
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
                KeyCode::Esc => self.screen = Screen::Accounts,
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

fn login_error_context(intent: &LoginIntent) -> &'static str {
    match intent {
        LoginIntent::Add => "Login failed",
        LoginIntent::Reauthenticate { .. } => "Reauthentication not completed",
        LoginIntent::Reconnect { .. } => "Reconnection not completed",
    }
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
    let mut candidates: Vec<(std::ffi::OsString, Vec<std::ffi::OsString>)> = Vec::new();
    // Prefer the explicitly configured browser so the URL is forwarded to its
    // running instance, then use desktop URL handlers as fallbacks.
    if let Some(browser) = env::var_os("BROWSER") {
        candidates.push((browser, vec![url.into()]));
    }
    candidates.push(("gio".into(), vec!["open".into(), url.into()]));
    candidates.push(("xdg-open".into(), vec![url.into()]));
    // Direct browser fallbacks request a new tab where supported, which gives
    // window managers a stronger opportunity to activate the browser window.
    for browser in ["brave", "google-chrome-stable", "google-chrome", "chromium"] {
        candidates.push((browser.into(), vec!["--new-tab".into(), url.into()]));
    }
    candidates.push(("firefox".into(), vec![url.into()]));

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
    use arqen::{Account, AccountStore, ConnectionState};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

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
            intent: LoginIntent::Add,
        };
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
    }
}

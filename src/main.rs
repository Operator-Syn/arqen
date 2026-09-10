use anyhow::{Context, Result};
use arqen::{
    Account, AccountStore,
    auth::{GoogleOAuth, parse_callback, token_key},
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
    },
    Redirect {
        oauth: Option<GoogleOAuth>,
        input: String,
    },
    Error(String),
    ConfirmQuit,
}

struct App {
    store: AccountStore,
    accounts: Vec<Account>,
    selected: usize,
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
            screen: Screen::Accounts,
            notice: None,
            clipboard: None,
        })
    }

    fn show_error(&mut self, context: &str, error: impl std::fmt::Display) {
        self.notice = None;
        self.screen = Screen::Error(format!("{context}\n\n{error}"));
    }

    fn start_login(&mut self) {
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

    fn finish_login(&mut self, oauth: &mut GoogleOAuth, input: &str) {
        let result = (|| -> Result<Account> {
            let profile = oauth
                .finish(parse_callback(input).context("parse pasted Google redirect URL")?)
                .context("complete Google login")?;
            let account = Account {
                id: uuid::Uuid::new_v4().to_string(),
                subject: profile.sub.clone(),
                email: profile.email,
                display_name: profile.name,
                token_key: Some(token_key(&profile.sub)),
            };
            self.store
                .upsert_google_account(&account)
                .context("save Google account metadata in SQLite")?;
            Ok(account)
        })();
        match result {
            Ok(account) => self.complete_login(account),
            Err(error) => self.show_error("Login failed", format_args!("{error:#}")),
        }
    }

    fn complete_login(&mut self, _account: Account) {
        match self.store.list_accounts() {
            Ok(accounts) => self.accounts = accounts,
            Err(error) => {
                self.show_error("Account connected, but refreshing accounts failed", error);
                return;
            }
        }
        self.selected = self.selected.min(self.accounts.len().saturating_sub(1));
        self.notice = Some("Google account connected successfully.".into());
        // This is the single success boundary for both automatic and manual
        // callback flows; replacing the screen guarantees the modal is gone
        // on the next frame.
        self.screen = Screen::Accounts;
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
        let (mut oauth, redirect_uri, callback) = match screen {
            Screen::Authorization {
                mut oauth,
                callback: Some(callback),
                ..
            } => (oauth.take(), callback.redirect_uri().to_owned(), callback),
            _ => return,
        };
        drop(callback);
        let input = format!("{redirect_uri}{target}");
        let Some(mut oauth) = oauth.take() else {
            self.show_error("Unable to finish login", "authorization state is missing");
            return;
        };
        self.finish_login(&mut oauth, &input);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match &mut self.screen {
            Screen::Accounts => match key.code {
                KeyCode::Char('a') => self.start_login(),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected = self.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.selected = (self.selected + 1).min(self.accounts.len().saturating_sub(1));
                }
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
                    };
                    self.notice = Some(format!("Open this URL in your browser:\n{url}"));
                }
                KeyCode::Esc => self.screen = Screen::Accounts,
                _ => {}
            },
            Screen::Redirect { oauth, input } => match key.code {
                KeyCode::Enter => {
                    let input = std::mem::take(input);
                    let Some(mut oauth) = oauth.take() else {
                        self.show_error("Unable to finish login", "authorization state is missing");
                        return;
                    };
                    self.finish_login(&mut oauth, &input);
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
            Screen::ConfirmQuit => {}
        }
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
                return true;
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
        match ui::mouse_target(area, app.accounts.len(), column, row, app.notice.as_deref()) {
            Some(ui::MouseTarget::Account(index)) => {
                app.selected = index;
                app.notice = Some(format!(
                    "Selected {}",
                    app.accounts[index]
                        .display_name
                        .as_deref()
                        .unwrap_or("Unnamed account")
                ));
            }
            Some(ui::MouseTarget::AddAccount) => app.start_login(),
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
                && !matches!(app.screen, Screen::ConfirmQuit)
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
        Event::Mouse(mouse)
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && handle_mouse(app, mouse.column, mouse.row) =>
        {
            return true;
        }
        _ => {}
    }
    false
}

mod ui;

#[cfg(test)]
mod tests {
    use super::{App, Screen, handle_event};
    use arqen::{Account, AccountStore};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn app_with_accounts() -> App {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "first".into(),
                subject: "subject-first".into(),
                email: "first@example.com".into(),
                display_name: Some("First Account".into()),
                token_key: Some("google/first".into()),
            })
            .unwrap();
        store
            .upsert_google_account(&Account {
                id: "second".into(),
                subject: "subject-second".into(),
                email: "second@example.com".into(),
                display_name: Some("Second Account".into()),
                token_key: Some("google/second".into()),
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
    fn enter_reports_selected_account_and_escape_closes_error() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.notice.as_deref().unwrap().contains("First Account"));
        app.screen = Screen::Error("test error".into());
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.screen, Screen::Accounts));
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
    fn successful_completion_replaces_any_login_screen() {
        let mut app = app_with_accounts();
        app.screen = Screen::Error("stale login modal".into());
        let account = Account {
            id: "completed".into(),
            subject: "subject-completed".into(),
            email: "completed@example.com".into(),
            display_name: Some("Completed".into()),
            token_key: Some("keyring:arqen:subject-completed".into()),
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
        };
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
    }
}

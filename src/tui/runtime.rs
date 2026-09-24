// SPDX-License-Identifier: MPL-2.0
pub(crate) fn run() -> Result<()> {
    let path = crate::config::database_path().with_context(|| "open application data directory")?;
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

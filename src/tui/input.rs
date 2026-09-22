fn handle_mouse(app: &mut App, column: u16, row: u16) -> bool {
    let Ok((width, height)) = crossterm::terminal::size() else {
        return false;
    };
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    if !matches!(app.screen, Screen::Accounts) {
        return handle_modal_mouse(app, area, column, row);
    }
    handle_account_mouse(app, area, column, row);
    false
}

fn handle_modal_mouse(app: &mut App, area: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    let Some(action) = ui::modal_action(area, &app.screen, column, row, app.notice.as_deref())
    else {
        return false;
    };
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
    if matches!(app.screen, Screen::ConfirmQuit) && matches!(action, ModalActionId::Cancel) {
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
    false
}

fn handle_account_mouse(
    app: &mut App,
    area: ratatui::layout::Rect,
    column: u16,
    row: u16,
) {
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
    let target = ui::mouse_target_with_focus(
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
    );
    apply_account_mouse_target(app, target);
}

fn apply_account_mouse_target(app: &mut App, target: Option<ui::MouseTarget>) {
    match target {
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
            if let Some(subject) = app.selected_subject() {
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
            if let Some(subject) = app.reconnectable_subject() {
                app.start_login(LoginIntent::Reconnect { subject });
            }
        }
        Some(ui::MouseTarget::ConnectionBadge) => handle_badge_mouse(app),
        Some(ui::MouseTarget::Focus) => {
            app.pane_focus = match app.pane_focus {
                PaneFocus::Accounts => PaneFocus::Details,
                PaneFocus::Details => PaneFocus::Accounts,
            };
        }
        None => {}
    }
}

fn handle_badge_mouse(app: &mut App) {
    app.pane_focus = PaneFocus::Details;
    match app
        .accounts
        .get(app.selected)
        .map(|account| account.connection_state)
    {
        Some(ConnectionState::Connected) => app.confirm_disconnect(false),
        Some(ConnectionState::Indeterminate) => app.confirm_disconnect(true),
        Some(ConnectionState::Disconnected) => {
            if let Some(subject) = app.selected_subject() {
                app.start_login(LoginIntent::Reconnect { subject });
            }
        }
        None => {}
    }
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

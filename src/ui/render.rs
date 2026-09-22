#[derive(Debug, Clone, Copy)]
struct Areas {
    header: Rect,
    accounts: Rect,
    details: Rect,
    footer: Rect,
    mode: UiMode,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    frame: &mut Frame<'_>,
    accounts: &[Account],
    selected: usize,
    target_subject: Option<&str>,
    pane_focus: PaneFocus,
    accounts_scroll: &mut usize,
    details_scroll: &mut usize,
    screen: &Screen,
    notice: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BACKGROUND)),
        area,
    );

    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(area, accounts.len(), selected_state, pane_focus, notice);
    chrome::render_header(frame, areas.header, accounts, areas.mode);
    accounts::render_account_list(
        frame,
        areas.accounts,
        accounts,
        selected,
        target_subject,
        areas.mode,
        pane_focus == PaneFocus::Accounts,
        accounts_scroll,
    );
    accounts::render_account_details(
        frame,
        areas.details,
        accounts.get(selected),
        target_subject,
        areas.mode,
        pane_focus == PaneFocus::Details,
        details_scroll,
    );
    chrome::render_footer(
        frame,
        areas.footer,
        notice,
        areas.mode,
        selected_state,
        pane_focus,
    );

    match screen {
        Screen::Accounts => {}
        Screen::Authorization {
            url,
            callback,
            remote,
            intent,
            ..
        } => {
            dialogs::render_authorization(
                frame,
                area,
                url,
                areas.mode == UiMode::Compact,
                callback.is_none(),
                *remote,
                matches!(
                    intent,
                    LoginIntent::Reauthenticate { .. } | LoginIntent::Reconnect { .. }
                ),
                matches!(intent, LoginIntent::Reconnect { .. }),
            );
        }
        Screen::Redirect { input, .. } => {
            dialogs::render_redirect(frame, area, notice, input, areas.mode == UiMode::Compact);
        }
        Screen::Error(message) => {
            dialogs::render_error(frame, area, message, areas.mode == UiMode::Compact);
        }
        Screen::ConfirmQuit => {
            dialogs::render_confirm_quit(frame, area, areas.mode == UiMode::Compact);
        }
        Screen::ConfirmDisconnect { email, retry, .. } => {
            dialogs::render_disconnect(frame, area, email, *retry, areas.mode == UiMode::Compact);
        }
    }
}

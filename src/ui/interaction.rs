#[cfg(test)]
pub(crate) fn mouse_target(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<MouseTarget> {
    mouse_target_with_focus(
        area,
        accounts,
        selected,
        column,
        row,
        InteractionContext {
            pane_focus: PaneFocus::Accounts,
            accounts_scroll: 0,
        },
        notice,
    )
}

pub(crate) fn mouse_target_with_focus(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    context: InteractionContext,
    notice: Option<&str>,
) -> Option<MouseTarget> {
    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(
        area,
        accounts.len(),
        selected_state,
        context.pane_focus,
        notice,
    );
    accounts::mouse_target_with_scroll(
        areas.accounts,
        accounts.len(),
        column,
        row,
        areas.mode,
        context.accounts_scroll,
    )
    .or_else(|| {
        chrome::reauthenticate_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Reauthenticate)
    })
    .or_else(|| {
        chrome::disconnect_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Disconnect)
    })
    .or_else(|| {
        chrome::login_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Login)
    })
    .or_else(|| {
        chrome::focus_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Focus)
    })
    .or_else(|| {
        (selected_state.is_some()
            && accounts::connection_badge_target(areas.details, areas.mode, column, row))
        .then_some(MouseTarget::ConnectionBadge)
    })
    .or_else(|| {
        (accounts.is_empty() && contains(areas.details, column, row))
            .then_some(MouseTarget::AddAccount)
    })
}

pub(crate) fn modal_action(
    area: Rect,
    screen: &Screen,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<modal::ModalActionId> {
    let compact = ui_mode(area) == UiMode::Compact;
    let spec = match screen {
        Screen::Authorization {
            url,
            callback,
            remote,
            intent,
            ..
        } => dialogs::authorization_spec(
            url,
            compact,
            callback.is_none(),
            *remote,
            matches!(
                intent,
                LoginIntent::Reauthenticate { .. } | LoginIntent::Reconnect { .. }
            ),
            matches!(intent, LoginIntent::Reconnect { .. }),
        ),
        Screen::Redirect { input, .. } => dialogs::redirect_spec(notice, input, compact),
        Screen::Error(message) => dialogs::error_spec(message, compact),
        Screen::ConfirmQuit => dialogs::confirm_quit_spec(compact),
        Screen::ConfirmDisconnect { email, retry, .. } => {
            dialogs::disconnect_spec(email, *retry, compact)
        }
        Screen::Accounts => return None,
    };
    modal::Modal::hit_test(area, &spec, column, row)
}

pub(crate) fn pane_at_with_notice(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<PaneFocus> {
    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(
        area,
        accounts.len(),
        selected_state,
        PaneFocus::Accounts,
        notice,
    );
    if contains(areas.accounts, column, row) {
        Some(PaneFocus::Accounts)
    } else if contains(areas.details, column, row) {
        Some(PaneFocus::Details)
    } else {
        None
    }
}

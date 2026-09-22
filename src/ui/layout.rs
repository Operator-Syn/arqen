pub(crate) fn pane_viewport(
    area: Rect,
    account_count: usize,
    selected_state: Option<ConnectionState>,
    pane: PaneFocus,
) -> usize {
    let areas = layout(area, account_count, selected_state, pane, None);
    match pane {
        PaneFocus::Accounts => {
            accounts::list_viewport_rows(areas.accounts, account_count, areas.mode)
        }
        PaneFocus::Details => accounts::details_viewport_rows(areas.details, areas.mode),
    }
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn layout(
    area: Rect,
    _account_count: usize,
    selected_state: Option<ConnectionState>,
    pane_focus: PaneFocus,
    notice: Option<&str>,
) -> Areas {
    let inset = proportional_inset(area);
    let content = Rect {
        x: area.x.saturating_add(inset),
        y: area.y.saturating_add(inset),
        width: area.width.saturating_sub(inset.saturating_mul(2)),
        height: area.height.saturating_sub(inset.saturating_mul(2)),
    };
    let mode = ui_mode(content);
    let vertical = Layout::vertical([
        Constraint::Length(chrome::header_height(content.width, mode)),
        Constraint::Min(1),
        Constraint::Length(chrome::footer_height(
            content.width,
            notice,
            mode,
            selected_state,
            pane_focus,
        )),
    ])
    .split(content);
    let body = if mode == UiMode::Wide {
        Layout::horizontal([
            Constraint::Percentage(39),
            Constraint::Percentage(1),
            Constraint::Fill(1),
        ])
        .split(vertical[1])
    } else {
        Layout::vertical([
            Constraint::Percentage(38),
            Constraint::Length(0),
            Constraint::Fill(1),
        ])
        .split(vertical[1])
    };
    Areas {
        header: vertical[0],
        accounts: body[0],
        details: body[2],
        footer: vertical[2],
        mode,
    }
}

fn proportional_inset(area: Rect) -> u16 {
    let basis = area.width.min(area.height);
    basis.saturating_div(24).clamp(1, 3)
}

fn ui_mode(area: Rect) -> UiMode {
    // Terminal-column breakpoints approximating common 576px/960px web breakpoints.
    if area.height < 20 || area.width < 72 {
        UiMode::Compact
    } else if area.width < 118 {
        UiMode::Narrow
    } else if u32::from(area.width) * 10 >= u32::from(area.height) * 37 {
        UiMode::Wide
    } else {
        UiMode::Narrow
    }
}

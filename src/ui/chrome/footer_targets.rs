pub(crate) fn reauthenticate_target(
    area: Rect,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    column: u16,
    row: u16,
) -> bool {
    action_target(area, mode, selected_state, "[r]", column, row)
}

pub(crate) fn disconnect_target(
    area: Rect,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    column: u16,
    row: u16,
) -> bool {
    action_target(area, mode, selected_state, "[d]", column, row)
}

pub(crate) fn login_target(
    area: Rect,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    column: u16,
    row: u16,
) -> bool {
    action_target(area, mode, selected_state, "[l]", column, row)
}

pub(crate) fn focus_target(
    area: Rect,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    column: u16,
    row: u16,
) -> bool {
    action_target(area, mode, selected_state, "[Tab]", column, row)
}

fn action_target(
    area: Rect,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    target: &str,
    column: u16,
    row: u16,
) -> bool {
    let Some(selected_state) = selected_state else {
        return false;
    };
    let inner = if mode == UiMode::Compact {
        area
    } else {
        Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    };
    let action_column = if mode == UiMode::Wide {
        wide_footer_columns(inner)[0]
    } else {
        inner
    };
    let tokens = match mode {
        UiMode::Compact => compact_footer_tokens(Some(selected_state)),
        UiMode::Narrow => narrow_footer_tokens(Some(selected_state)),
        UiMode::Wide => footer_tokens(Some(selected_state)),
    };
    let Some(token_start) = tokens
        .iter()
        .scan(0usize, |offset, (token, label)| {
            let current = *offset;
            *offset += token.chars().count() + 1 + label.chars().count() + 2;
            Some((token, current))
        })
        .find_map(|(token, offset)| (*token == target).then_some(offset))
    else {
        return false;
    };
    let width = usize::from(action_column.width.max(1));
    let line_offset = token_start / width;
    let column_offset = token_start % width;
    row == action_column
        .y
        .saturating_add(u16::try_from(line_offset).unwrap_or(u16::MAX))
        && usize::from(column.saturating_sub(action_column.x)) >= column_offset
        && usize::from(column.saturating_sub(action_column.x))
            < column_offset + target.chars().count()
}

fn footer_tokens(selected_state: Option<ConnectionState>) -> Vec<(&'static str, &'static str)> {
    let mut tokens = vec![("[a]", "add")];
    if let Some(state) = selected_state {
        match state {
            ConnectionState::Connected => tokens.push(("[d]", "disconnect")),
            ConnectionState::Disconnected => tokens.push(("[l]", "login")),
            ConnectionState::Indeterminate => {
                tokens.push(("[d]", "retry"));
                tokens.push(("[l]", "login"));
            }
        }
        tokens.push(("[r]", "reauth"));
    }
    tokens.extend([
        ("[Tab]", "focus"),
        ("[Wheel]", "scroll"),
        ("[j/k]", "select"),
        ("[Enter]", "inspect"),
        ("[q]", "quit"),
    ]);
    tokens
}

fn compact_footer_tokens(
    selected_state: Option<ConnectionState>,
) -> Vec<(&'static str, &'static str)> {
    let mut tokens = vec![("[a]", "add")];
    if let Some(state) = selected_state {
        match state {
            ConnectionState::Connected => {
                tokens.push(("[d]", "off"));
                tokens.push(("[r]", "reauth"));
            }
            ConnectionState::Disconnected => {
                tokens.push(("[l]", "login"));
                tokens.push(("[r]", "reauth"));
            }
            ConnectionState::Indeterminate => {
                tokens.push(("[d]", "retry"));
                tokens.push(("[l]", "login"));
            }
        }
    }
    tokens.extend([("[Tab]", "focus"), ("[Wheel]", "scroll")]);
    tokens
}

fn narrow_footer_tokens(
    selected_state: Option<ConnectionState>,
) -> Vec<(&'static str, &'static str)> {
    let mut tokens = vec![("[a]", "add")];
    if let Some(state) = selected_state {
        match state {
            ConnectionState::Connected => {
                tokens.push(("[d]", "disconnect"));
                tokens.push(("[r]", "reauth"));
            }
            ConnectionState::Disconnected => {
                tokens.push(("[l]", "login"));
                tokens.push(("[r]", "reauth"));
            }
            ConnectionState::Indeterminate => {
                tokens.push(("[d]", "retry"));
                tokens.push(("[l]", "login"));
                tokens.push(("[r]", "reauth"));
            }
        }
    }
    tokens.extend([("[Tab]", "focus"), ("[Wheel]", "scroll")]);
    tokens
}

fn connected_count(accounts: &[Account]) -> usize {
    accounts
        .iter()
        .filter(|account| account.connection_state == ConnectionState::Connected)
        .count()
}

fn wrapped_height(text: Text<'_>, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    let lines = text
        .lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum::<usize>();
    u16::try_from(lines).unwrap_or(u16::MAX).max(1)
}

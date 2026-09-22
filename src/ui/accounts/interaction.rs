pub(crate) fn connection_badge_target(area: Rect, mode: UiMode, column: u16, row: u16) -> bool {
    let badge = connection_badge_area(area, mode);
    column >= badge.x
        && column < badge.x.saturating_add(badge.width)
        && row >= badge.y
        && row < badge.y.saturating_add(badge.height)
}

pub(crate) fn scope_heading(account: &Account) -> &'static str {
    if account.connection_state == ConnectionState::Connected {
        "Granted scopes"
    } else {
        "Last confirmed scopes"
    }
}

fn panel(title: &str, border: ratatui::style::Color, _width: u16) -> Block<'static> {
    Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(theme::SURFACE))
}

fn detail_content(inner: Rect, width: u16) -> Rect {
    let inset = super::content_padding(width).saturating_add(1).min(3);
    Rect {
        x: inner.x.saturating_add(inset),
        width: inner.width.saturating_sub(inset.saturating_mul(2)),
        ..inner
    }
}

fn identity_height(mode: UiMode) -> u16 {
    if mode == UiMode::Wide { 4 } else { 3 }
}

fn render_panel_header(frame: &mut Frame<'_>, inner: Rect) {
    let header = Rect {
        height: 2.min(inner.height),
        ..inner
    };
    frame.render_widget(
        Paragraph::new("Selected account")
            .style(
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(theme::BORDER))
                    .style(Style::default().bg(theme::SURFACE))
                    .padding(Padding::horizontal(1)),
            ),
        header,
    );
}

fn detail_line_styled(
    label: &str,
    value: &str,
    color: ratatui::style::Color,
    label_width: usize,
) -> Line<'static> {
    let label_column = format!(" {label:<label_width$}");
    Line::from(vec![
        Span::styled(
            format!("{label_column}{}", " ".repeat(DETAIL_COLUMN_GAP)),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled(
            format!("│{}", " ".repeat(DETAIL_COLUMN_GAP)),
            Style::default().fg(theme::BORDER),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn section_heading(text: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            text.to_owned(),
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
    ])
}

fn separator(width: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width as usize),
        Style::default().fg(theme::BORDER),
    ))
}

pub(crate) fn mouse_target_with_scroll(
    area: Rect,
    count: usize,
    column: u16,
    row: u16,
    mode: UiMode,
    scroll_offset: usize,
) -> Option<super::MouseTarget> {
    if column < area.x
        || column >= area.x.saturating_add(area.width)
        || row < area.y
        || row >= area.y.saturating_add(area.height)
    {
        return None;
    }
    if count == 0 {
        return Some(super::MouseTarget::AddAccount);
    }
    let list_top = if mode != UiMode::Wide { 2 } else { 3 };
    let row_start = area.y.saturating_add(1).saturating_add(list_top);
    if column <= area.x
        || column >= area.x.saturating_add(area.width.saturating_sub(1))
        || row < row_start
    {
        return None;
    }
    let row_height = row_height(area, count, mode);
    let offset = row.saturating_sub(row_start);
    let local_index = usize::from(offset / row_height);
    let index = scroll_offset.saturating_add(local_index);
    if index < count {
        Some(super::MouseTarget::Account(index))
    } else if index == count {
        Some(super::MouseTarget::AddAccount)
    } else {
        None
    }
}

fn row_height(area: Rect, count: usize, mode: UiMode) -> u16 {
    if count == 0 {
        return 1;
    }
    let available = area.height.saturating_sub(2);
    let per_account = available / u16::try_from(count).unwrap_or(u16::MAX);
    if mode == UiMode::Wide && per_account >= 4 {
        4
    } else if mode != UiMode::Compact && per_account >= 2 {
        2
    } else {
        1
    }
}

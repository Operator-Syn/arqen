fn render_stacked_footer<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    actions: Line<'static>,
    notice: Line<'a>,
) {
    let action_height = wrapped_height(Text::from(actions.clone()), area.width);
    let action_area = Rect {
        height: action_height.min(area.height),
        ..area
    };
    frame.render_widget(
        Paragraph::new(actions).wrap(Wrap { trim: true }),
        action_area,
    );
    let notice_area = Rect {
        y: area.y.saturating_add(action_area.height),
        height: area.height.saturating_sub(action_area.height),
        ..area
    };
    if notice_area.height > 0 {
        render_notice(frame, notice_area, notice);
    }
}

fn render_notice<'a>(frame: &mut Frame<'_>, area: Rect, notice: Line<'a>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let notice_height = wrapped_height(Text::from(notice.clone()), area.width).min(area.height);
    let notice_area = Rect {
        y: area
            .y
            .saturating_add(area.height.saturating_sub(notice_height) / 2),
        height: notice_height,
        ..area
    };
    frame.render_widget(
        Paragraph::new(notice)
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: true }),
        notice_area,
    );
}

pub(crate) fn header_height(width: u16, mode: UiMode) -> u16 {
    if mode == UiMode::Compact {
        return wrapped_height(
            Text::from(Line::from(vec![
                title_line(),
                Span::styled("  /  Identity Vault", Style::default().fg(theme::MUTED)),
            ])),
            width,
        );
    }
    let inner = width
        .saturating_sub(2 + super::content_padding(width).saturating_mul(2))
        .max(1);
    if mode == UiMode::Narrow {
        let left = wrapped_height(
            Text::from(vec![
                Line::from(title_line()),
                Line::from(Span::styled(
                    " Identity Vault",
                    Style::default().fg(theme::MUTED),
                )),
            ]),
            inner.saturating_mul(60) / 100,
        );
        let status = wrapped_height(
            Text::from(vec![Line::from("LOCAL SESSION"), Line::from("0 connected")]),
            inner.saturating_mul(40) / 100,
        );
        return left.max(status).saturating_add(2);
    }
    let left = wrapped_height(
        Text::from(vec![
            Line::from(title_line()),
            Line::from(Span::styled(
                " Identity Vault",
                Style::default().fg(theme::MUTED),
            )),
        ]),
        inner.saturating_mul(60) / 100,
    );
    let right = wrapped_height(
        Text::from(vec![Line::from("LOCAL SESSION"), Line::from("0 connected")]),
        inner.saturating_mul(40) / 100,
    );
    left.max(right).saturating_add(2)
}

pub(crate) fn footer_height(
    width: u16,
    notice: Option<&str>,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    pane_focus: PaneFocus,
) -> u16 {
    let actions = Text::from(Line::from(footer_actions(selected_state, pane_focus)));
    let notice = notice.unwrap_or("Select an account to inspect its connection.");
    let notice_text = notice_label(notice);
    if mode == UiMode::Compact {
        return wrapped_height(
            Text::from(Line::from(compact_footer_actions(selected_state))),
            width,
        )
        .saturating_add(wrapped_height(Text::from(notice_text.as_str()), width));
    }
    let inner = width
        .saturating_sub(2 + super::content_padding(width).saturating_mul(2))
        .max(1);
    if mode == UiMode::Narrow {
        let actions_height = wrapped_height(
            Text::from(Line::from(narrow_footer_actions(selected_state))),
            inner,
        );
        let notice_height = wrapped_height(Text::from(notice_text.as_str()), inner);
        return actions_height
            .saturating_add(notice_height)
            .saturating_add(2);
    }
    let [action_column, _, notice_column] = wide_footer_columns(Rect::new(0, 0, inner, 1));
    let action_height = wrapped_height(actions, action_column.width);
    let notice_height = wrapped_height(
        Text::from(notice_text.as_str()),
        notice_column.width.saturating_sub(1),
    );
    action_height.max(notice_height).saturating_add(2)
}

fn notice_label(notice: &str) -> String {
    if is_positive_notice(notice) {
        format!("[OK] {notice}")
    } else {
        notice.to_owned()
    }
}

fn notice_line<'a>(notice: &str, rendered: &'a str) -> Line<'a> {
    let color = if is_positive_notice(notice) {
        theme::SUCCESS
    } else {
        theme::MUTED
    };
    Line::from(Span::styled(
        rendered,
        Style::default()
            .fg(color)
            .add_modifier(if is_positive_notice(notice) {
                ratatui::style::Modifier::BOLD
            } else {
                ratatui::style::Modifier::empty()
            }),
    ))
}

fn is_positive_notice(notice: &str) -> bool {
    notice.starts_with("Authorization URL copied")
        || notice.starts_with("Authorization URL opened")
        || notice.starts_with("Browser opened")
        || notice.starts_with("Dedicated Google login window opened")
        || notice.starts_with("Google account connected")
        || notice.starts_with("MCP target set")
        || notice.starts_with("MCP target cleared")
        || notice.starts_with("Quit cancelled")
        || notice.starts_with("Selected ")
}

fn title_line() -> Span<'static> {
    Span::styled(
        "ARQEN",
        Style::default()
            .fg(theme::PRIMARY)
            .add_modifier(ratatui::style::Modifier::BOLD),
    )
}

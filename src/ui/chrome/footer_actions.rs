// SPDX-License-Identifier: MPL-2.0
fn footer_actions(
    selected_state: Option<ConnectionState>,
    pane_focus: PaneFocus,
) -> Vec<Span<'static>> {
    let mut actions = vec![
        Span::styled(
            "[a]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" add  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[t]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" target  ", Style::default().fg(theme::TEXT)),
    ];
    if let Some(state) = selected_state {
        let (shortcut, label) = match state {
            ConnectionState::Connected => ("[d]", " disconnect  "),
            ConnectionState::Disconnected => ("[l]", " login  "),
            ConnectionState::Indeterminate => ("[d]", " retry  "),
        };
        actions.extend([
            Span::styled(
                shortcut,
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled(label, Style::default().fg(theme::TEXT)),
        ]);
        if state == ConnectionState::Indeterminate {
            actions.extend([
                Span::styled(
                    "[l]",
                    Style::default()
                        .fg(theme::PRIMARY)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::styled(" login  ", Style::default().fg(theme::TEXT)),
            ]);
        }
        actions.extend([
            Span::styled(
                "[r]",
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled(" reauth  ", Style::default().fg(theme::TEXT)),
        ]);
    }
    actions.extend([
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" focus  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[Wheel]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" scroll  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            format!(
                "({})  ",
                match pane_focus {
                    PaneFocus::Accounts => "accounts focused",
                    PaneFocus::Details => "details focused",
                }
            ),
            Style::default().fg(theme::MUTED),
        ),
        Span::styled(
            "[j/k]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" select  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[Enter]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" inspect  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[q]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled("\u{00a0}quit", Style::default().fg(theme::TEXT)),
    ]);
    actions
}

fn wide_footer_columns(inner: Rect) -> [Rect; 3] {
    let columns = Layout::horizontal([
        Constraint::Percentage(60),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(inner);
    [columns[0], columns[1], columns[2]]
}

fn compact_footer_actions(selected_state: Option<ConnectionState>) -> Vec<Span<'static>> {
    let mut actions = vec![
        Span::styled(
            "[a]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" add  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[t]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" target  ", Style::default().fg(theme::TEXT)),
    ];
    if let Some(state) = selected_state {
        let entries: &[(&str, &str)] = match state {
            ConnectionState::Connected => &[("[d]", " off  "), ("[r]", " reauth  ")],
            ConnectionState::Disconnected => &[("[l]", " login  "), ("[r]", " reauth  ")],
            ConnectionState::Indeterminate => &[("[d]", " retry  "), ("[l]", " login  ")],
        };
        for (shortcut, label) in entries {
            actions.push(Span::styled(
                *shortcut,
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
            actions.push(Span::styled(*label, Style::default().fg(theme::TEXT)));
        }
    }
    actions.extend([
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" focus  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[Wheel]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" scroll", Style::default().fg(theme::TEXT)),
    ]);
    actions
}

fn narrow_footer_actions(selected_state: Option<ConnectionState>) -> Vec<Span<'static>> {
    let mut actions = vec![
        Span::styled(
            "[a]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" add  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[t]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" target  ", Style::default().fg(theme::TEXT)),
    ];
    if let Some(state) = selected_state {
        let entries: &[(&str, &str)] = match state {
            ConnectionState::Connected => &[("[d]", " disconnect  "), ("[r]", " reauth  ")],
            ConnectionState::Disconnected => &[("[l]", " login  "), ("[r]", " reauth  ")],
            ConnectionState::Indeterminate => &[
                ("[d]", " retry  "),
                ("[l]", " login  "),
                ("[r]", " reauth  "),
            ],
        };
        for (shortcut, label) in entries {
            actions.push(Span::styled(
                *shortcut,
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
            actions.push(Span::styled(*label, Style::default().fg(theme::TEXT)));
        }
    }
    actions.extend([
        Span::styled(
            "[Tab]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" focus  ", Style::default().fg(theme::TEXT)),
        Span::styled(
            "[Wheel]",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(" scroll", Style::default().fg(theme::TEXT)),
    ]);
    actions
}

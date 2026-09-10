use super::{PaneFocus, UiMode, theme};
use arqen::{Account, ConnectionState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::{Line, Span, Style, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

pub(crate) fn render_header(frame: &mut Frame<'_>, area: Rect, accounts: &[Account], mode: UiMode) {
    let title = title_line();
    if mode == UiMode::Compact || area.height < 3 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                title,
                Span::styled("  /  Identity Vault", Style::default().fg(theme::MUTED)),
            ]))
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(super::content_padding(area.width)))
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::SURFACE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let columns =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Fill(1)]).split(inner);

    if mode == UiMode::Narrow {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(title),
                Line::from(Span::styled(
                    " Identity Vault",
                    Style::default().fg(theme::MUTED),
                )),
            ])
            .wrap(Wrap { trim: true }),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "LOCAL SESSION",
                    Style::default()
                        .fg(theme::PRIMARY)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("{} connected", connected_count(accounts)),
                    Style::default().fg(theme::MUTED),
                )),
            ])
            .alignment(Alignment::Right)
            .wrap(Wrap { trim: true }),
            columns[1],
        );
        return;
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(title),
            Line::from(Span::styled(
                " Identity Vault",
                Style::default().fg(theme::MUTED),
            )),
        ])
        .wrap(Wrap { trim: true }),
        columns[0],
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "LOCAL SESSION",
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!("{} connected", connected_count(accounts)),
                Style::default().fg(theme::MUTED),
            )),
        ])
        .alignment(Alignment::Right)
        .wrap(Wrap { trim: true }),
        columns[1],
    );
}

pub(crate) fn render_footer(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: Option<&str>,
    mode: UiMode,
    selected_state: Option<ConnectionState>,
    pane_focus: PaneFocus,
) {
    let notice = notice.unwrap_or("Select an account to inspect its connection.");
    let notice_text = notice_label(notice);
    let paragraph = Paragraph::new(Line::from(footer_actions(selected_state, pane_focus)))
        .wrap(Wrap { trim: true });
    if mode == UiMode::Compact || area.height < 3 {
        render_stacked_footer(
            frame,
            area,
            Line::from(compact_footer_actions(selected_state)),
            notice_line(notice, &notice_text),
        );
        return;
    }

    if mode == UiMode::Narrow {
        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(super::content_padding(area.width)))
            .border_style(Style::default().fg(theme::BORDER))
            .style(Style::default().bg(theme::SURFACE));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        render_stacked_footer(
            frame,
            inner,
            Line::from(narrow_footer_actions(selected_state)),
            notice_line(notice, &notice_text),
        );
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(super::content_padding(area.width)))
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::SURFACE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [action_column, separator_column, notice_column] = wide_footer_columns(inner);
    frame.render_widget(paragraph, action_column);
    frame.render_widget(
        Paragraph::new(
            (0..separator_column.height)
                .map(|_| Line::from(Span::styled("│", Style::default().fg(theme::BORDER))))
                .collect::<Vec<_>>(),
        ),
        separator_column,
    );
    let notice_column = Rect {
        x: notice_column.x.saturating_add(1),
        width: notice_column.width.saturating_sub(1),
        ..notice_column
    };
    render_notice(frame, notice_column, notice_line(notice, &notice_text));
}

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

#[cfg(test)]
mod tests {
    use super::{PaneFocus, UiMode, render_footer};
    use arqen::ConnectionState;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    #[test]
    fn stacked_notice_is_vertically_centered() {
        let area = Rect::new(0, 0, 60, 7);
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_footer(
                    frame,
                    area,
                    Some("Status complete"),
                    UiMode::Narrow,
                    Some(ConnectionState::Connected),
                    PaneFocus::Accounts,
                );
            })
            .expect("render footer");

        let notice_row = (0..area.height)
            .find(|row| {
                (0..area.width)
                    .map(|column| {
                        terminal
                            .backend()
                            .buffer()
                            .cell((column, *row))
                            .expect("notice cell")
                            .symbol()
                    })
                    .collect::<String>()
                    .contains("Status complete")
            })
            .expect("notice row");
        assert_eq!(notice_row, 4);
    }
}

use super::{UiMode, theme};
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
) {
    let notice = notice.unwrap_or("Select an account to inspect its connection.");
    let notice_text = notice_label(notice);
    let paragraph =
        Paragraph::new(Line::from(footer_actions(selected_state))).wrap(Wrap { trim: true });
    if mode == UiMode::Compact || area.height < 3 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(footer_actions(selected_state)),
                notice_line(notice, &notice_text),
            ])
            .wrap(Wrap { trim: true }),
            area,
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
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(footer_actions(selected_state)),
                notice_line(notice, &notice_text),
            ])
            .wrap(Wrap { trim: true }),
            inner,
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
    frame.render_widget(paragraph, columns[0]);
    frame.render_widget(
        Paragraph::new(notice_line(notice, &notice_text))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true }),
        columns[1],
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
) -> u16 {
    let actions = Text::from(Line::from(footer_actions(selected_state)));
    let notice = notice.unwrap_or("Select an account to inspect its connection.");
    let notice_text = notice_label(notice);
    if mode == UiMode::Compact {
        return wrapped_height(actions, width)
            .saturating_add(wrapped_height(Text::from(notice_text.as_str()), width));
    }
    let inner = width
        .saturating_sub(2 + super::content_padding(width).saturating_mul(2))
        .max(1);
    if mode == UiMode::Narrow {
        let actions_height = wrapped_height(
            Text::from(Line::from(footer_actions(selected_state))),
            inner,
        );
        let notice_height = wrapped_height(Text::from(notice_text.as_str()), inner);
        return actions_height
            .saturating_add(notice_height)
            .saturating_add(2);
    }
    let action_height = wrapped_height(actions, inner.saturating_mul(60) / 100);
    let notice_height = wrapped_height(
        Text::from(notice_text.as_str()),
        inner.saturating_mul(40) / 100,
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

fn footer_actions(selected_state: Option<ConnectionState>) -> Vec<Span<'static>> {
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
        Span::styled(" quit", Style::default().fg(theme::TEXT)),
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
        Layout::horizontal([Constraint::Percentage(60), Constraint::Fill(1)]).split(inner)[0]
    } else {
        inner
    };
    let tokens = footer_tokens(Some(selected_state));
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
    tokens.extend([("[j/k]", "select"), ("[Enter]", "inspect"), ("[q]", "quit")]);
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

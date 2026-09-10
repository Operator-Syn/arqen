use super::{UiMode, theme};
use arqen::Account;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    prelude::{Alignment, Line, Span, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
};

pub(crate) fn render_account_list(
    frame: &mut Frame<'_>,
    area: Rect,
    accounts: &[Account],
    selected: usize,
    mode: UiMode,
) {
    let row_height = row_height(area, accounts.len(), mode);
    let list_inset = super::content_padding(area.width).saturating_add(1).min(3);
    let list_top = if mode == UiMode::Compact || accounts.is_empty() {
        2
    } else {
        3
    };
    let row_width = area
        .width
        .saturating_sub(2)
        .saturating_sub(list_inset.saturating_mul(2))
        .saturating_sub(2);
    let mut items: Vec<ListItem<'_>> = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let name = account.email.as_str();
            let marker = if index == selected { ">" } else { " " };
            let card_padding: usize = if row_height >= 4 { 2 } else { 1 };
            if row_height >= 2 {
                let prefix = format!("{}{marker}  ●  {name}", " ".repeat(card_padding));
                let provider_gap = row_width
                    .saturating_sub(prefix.chars().count() as u16)
                    .saturating_sub((6 + card_padding) as u16)
                    as usize;
                let primary = Line::from(vec![
                    Span::styled(
                        format!("{}{marker}  ", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                    Span::styled("●", Style::default().fg(theme::SUCCESS)),
                    Span::styled(format!("  {name}"), Style::default().fg(theme::TEXT)),
                    Span::styled(
                        format!("{:provider_gap$}Google{}", "", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                ]);
                let secondary = Line::from(vec![
                    Span::raw(" ".repeat(card_padding + 6)),
                    Span::styled("Gmail read-only", Style::default().fg(theme::MUTED)),
                ]);
                if row_height >= 4 {
                    ListItem::new(vec![Line::from(""), primary, secondary, Line::from("")])
                } else {
                    ListItem::new(vec![primary, secondary])
                }
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}{marker}  ", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                    Span::styled("●", Style::default().fg(theme::SUCCESS)),
                    Span::styled(
                        format!("  {name}{}", " ".repeat(card_padding)),
                        Style::default().fg(theme::TEXT),
                    ),
                ]))
            }
        })
        .collect();

    if accounts.is_empty() {
        items.push(ListItem::new(if row_height == 2 {
            vec![
                Line::from(Span::styled(
                    "No accounts yet",
                    Style::default().fg(theme::TEXT),
                )),
                Line::from(Span::styled(
                    "Press a to connect Google",
                    Style::default().fg(theme::MUTED),
                )),
            ]
        } else {
            vec![Line::from(Span::styled(
                "No accounts yet",
                Style::default().fg(theme::TEXT),
            ))]
        }));
    } else {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                "+  ",
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled("Add another account", Style::default().fg(theme::PRIMARY)),
        ])));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::SURFACE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Accounts",
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "{:>width$}",
                    format!("{} / {}", accounts.len(), accounts.len()),
                    width = usize::from(inner.width.saturating_sub(10))
                ),
                Style::default().fg(theme::TEXT),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme::BORDER))
                .style(Style::default().bg(theme::SURFACE))
                .padding(Padding::horizontal(1)),
        ),
        Rect {
            height: 2.min(inner.height),
            ..inner
        },
    );
    let list_area = Rect {
        x: inner.x.saturating_add(list_inset),
        y: inner.y.saturating_add(list_top),
        width: inner.width.saturating_sub(list_inset.saturating_mul(2)),
        height: inner.height.saturating_sub(list_top),
    };
    let list = List::new(items)
        .block(Block::default().padding(Padding::horizontal(1)))
        .highlight_style(Style::default().bg(theme::SELECTION))
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(if accounts.is_empty() {
        None
    } else {
        Some(selected.min(accounts.len() - 1))
    });
    frame.render_stateful_widget(list, list_area, &mut state);
}

pub(crate) fn render_account_details(
    frame: &mut Frame<'_>,
    area: Rect,
    account: Option<&Account>,
    mode: UiMode,
) {
    let Some(account) = account else {
        let block = panel("", theme::BORDER, area.width);
        let inner = block.inner(area);
        let content = detail_content(inner, area.width);
        frame.render_widget(block, area);
        render_panel_header(frame, inner);
        frame.render_widget(
            Paragraph::new(if mode == UiMode::Compact {
                vec![
                    Line::from(Span::styled(
                        "NO ACCOUNT SELECTED",
                        Style::default()
                            .fg(theme::PRIMARY)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        "Press [a] to connect Google.",
                        Style::default().fg(theme::TEXT),
                    )),
                ]
            } else {
                vec![
                    Line::from(Span::styled(
                        "NO ACCOUNT SELECTED",
                        Style::default()
                            .fg(theme::PRIMARY)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Connect a Google identity to get started.",
                        Style::default().fg(theme::TEXT),
                    )),
                    Line::from(Span::styled(
                        "The app requests Gmail read-only access.",
                        Style::default().fg(theme::MUTED),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "[a]  CONNECT GOOGLE ACCOUNT",
                        Style::default()
                            .fg(theme::BACKGROUND)
                            .bg(theme::PRIMARY)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    )),
                ]
            })
            .alignment(Alignment::Center)
            .block(Block::default().style(Style::default().bg(theme::SURFACE))),
            Rect {
                y: content.y.saturating_add(2),
                height: content.height.saturating_sub(2),
                ..content
            },
        );
        return;
    };

    let block = panel("", theme::BORDER, area.width);
    let inner = block.inner(area);
    let content = detail_content(inner, area.width);
    frame.render_widget(block, area);
    render_panel_header(frame, inner);
    let panel_header_height = 2;
    let identity_height = if mode == UiMode::Compact { 3 } else { 4 };
    let header_area = Rect {
        y: content.y.saturating_add(panel_header_height),
        height: identity_height.min(content.height.saturating_sub(panel_header_height)),
        ..content
    };
    frame.render_widget(
        Paragraph::new(account.email.clone()).style(
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Rect {
            x: header_area.x.saturating_add(1),
            y: header_area.y.saturating_add(1),
            width: header_area.width.saturating_sub(19),
            height: 1.min(header_area.height.saturating_sub(1)),
        },
    );
    let badge_area = Rect {
        x: header_area
            .x
            .saturating_add(header_area.width.saturating_sub(17)),
        y: header_area.y,
        width: 17.min(header_area.width),
        height: 3.min(header_area.height),
    };
    frame.render_widget(
        Paragraph::new("● CONNECTED")
            .style(
                Style::default()
                    .fg(theme::SUCCESS)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::SUCCESS)),
            ),
        badge_area,
    );

    let mut connection_lines = vec![
        separator(content.width),
        Line::from(Span::styled(
            "Connection details",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )),
    ];
    let details = [
        detail_line("Provider", "Google"),
        detail_line("Access scope", "Gmail read-only"),
        detail_line_colored("Credential", "Protected (in OS keyring)", theme::SUCCESS),
        detail_line(
            "Keyring reference",
            account.token_key.as_deref().unwrap_or("Unavailable"),
        ),
    ];
    let details_len = details.len();
    for (index, line) in details.into_iter().enumerate() {
        if mode == UiMode::Wide {
            connection_lines.push(Line::from(""));
        }
        connection_lines.push(line);
        if mode == UiMode::Wide && index + 1 == details_len {
            connection_lines.push(Line::from(""));
        }
    }
    connection_lines.push(separator(content.width));
    let body = Rect {
        y: content
            .y
            .saturating_add(panel_header_height)
            .saturating_add(identity_height),
        height: content
            .height
            .saturating_sub(panel_header_height)
            .saturating_sub(identity_height),
        ..content
    };
    let additional = mode != UiMode::Compact;
    let sections = Layout::vertical([
        Constraint::Length(connection_lines.len() as u16),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(if additional { 5 } else { 0 }),
    ])
    .split(body);
    frame.render_widget(Paragraph::new(connection_lines), sections[0]);
    frame.render_widget(
        Paragraph::new("Scopes").style(
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        sections[1],
    );
    frame.render_widget(
        Paragraph::new("Gmail read-only")
            .style(Style::default().fg(theme::TEXT))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::BORDER))
                    .padding(Padding::horizontal(1)),
            ),
        sections[2],
    );
    if additional {
        let mut additional_lines = vec![
            separator(sections[4].width),
            Line::from(Span::styled(
                "Additional information",
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )),
        ];
        for line in [
            detail_line("Account ID", &account.id),
            detail_line("Type", "Personal"),
            detail_line_colored("Status", "Connected", theme::SUCCESS),
        ] {
            if mode == UiMode::Wide {
                additional_lines.push(Line::from(""));
            }
            additional_lines.push(line);
        }
        frame.render_widget(Paragraph::new(additional_lines), sections[4]);
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

fn detail_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<16}: "), Style::default().fg(theme::MUTED)),
        Span::styled(value.to_string(), Style::default().fg(theme::TEXT)),
    ])
}

fn detail_line_colored(label: &str, value: &str, color: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<16}: "), Style::default().fg(theme::MUTED)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn separator(width: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width as usize),
        Style::default().fg(theme::BORDER),
    ))
}

pub(crate) fn mouse_target(
    area: Rect,
    count: usize,
    column: u16,
    row: u16,
    mode: UiMode,
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
    let list_top = if mode == UiMode::Compact { 2 } else { 3 };
    let row_start = area.y.saturating_add(1).saturating_add(list_top);
    if column <= area.x
        || column >= area.x.saturating_add(area.width.saturating_sub(1))
        || row < row_start
    {
        return None;
    }
    let row_height = row_height(area, count, mode);
    let offset = row.saturating_sub(row_start);
    let index = usize::from(offset / row_height);
    if index < count {
        Some(super::MouseTarget::Account(index))
    } else if index == count
        && offset
            == u16::try_from(count)
                .unwrap_or(u16::MAX)
                .saturating_mul(row_height)
    {
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

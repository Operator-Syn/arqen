use super::{UiMode, theme};
use arqen::{Account, ConnectionState, GMAIL_READONLY_SCOPE};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Alignment, Line, Span, Style},
    widgets::{
        Block, Borders, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

const EMAIL_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.email";
const PROFILE_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.profile";
const CONNECTION_BADGE_WIDTH: u16 = 21;
const SCROLLBAR_TRACK_WIDTH: u16 = 1;
const SCROLLBAR_PADDING: u16 = 1;
const SCROLLBAR_RESERVED_WIDTH: u16 = SCROLLBAR_TRACK_WIDTH + SCROLLBAR_PADDING;
const DETAIL_COLUMN_GAP: usize = 2;

fn status_presentation(
    state: ConnectionState,
) -> (&'static str, &'static str, ratatui::style::Color) {
    match state {
        ConnectionState::Connected => ("●", "CONNECTED", theme::SUCCESS),
        ConnectionState::Disconnected => ("○", "DISCONNECTED", theme::WARNING),
        ConnectionState::Indeterminate => ("!", "UNKNOWN", theme::DANGER),
    }
}

pub(crate) fn status_label(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Connected => "Connected",
        ConnectionState::Disconnected => "Disconnected",
        ConnectionState::Indeterminate => "Connection unknown",
    }
}

pub(crate) fn scope_summary(account: &Account) -> String {
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return "Scopes unverified".into();
    };
    let mut labels = Vec::new();
    let mut unknown = 0usize;
    for scope in scopes {
        if let Some(label) = friendly_scope(scope) {
            labels.push(label.to_owned());
        } else {
            unknown += 1;
        }
    }
    labels.sort();
    labels.dedup();
    if unknown > 0 {
        if labels.is_empty() {
            format!("{unknown} unrecognized scope(s)")
        } else {
            format!("{} + {unknown} other", labels.join(" · "))
        }
    } else if labels.is_empty() {
        "No scopes recorded".into()
    } else {
        labels.join(" · ")
    }
}

fn friendly_scope(scope: &str) -> Option<&'static str> {
    match scope {
        "openid" => Some("OpenID identity"),
        "email" | EMAIL_SCOPE => Some("Email address"),
        "profile" | PROFILE_SCOPE => Some("Basic profile"),
        GMAIL_READONLY_SCOPE => Some("Gmail read-only"),
        _ => None,
    }
}

fn scope_lines(account: &Account) -> Vec<Line<'static>> {
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return vec![Line::from(Span::styled(
            "Scopes unverified — reauthenticate to record Google's grant.",
            Style::default().fg(theme::WARNING),
        ))];
    };

    let mut lines: Vec<Line<'static>> = scopes
        .iter()
        .map(|scope| {
            let text = friendly_scope(scope)
                .map(|label| format!("{label} — {scope}"))
                .unwrap_or_else(|| scope.clone());
            Line::from(Span::styled(text, Style::default().fg(theme::TEXT)))
        })
        .collect();
    if account.connection_state == ConnectionState::Disconnected {
        lines.insert(
            0,
            Line::from(Span::styled(
                "Provider grant revoked — reconnect to restore access.",
                Style::default().fg(theme::WARNING),
            )),
        );
    } else if account.connection_state == ConnectionState::Indeterminate {
        lines.insert(
            0,
            Line::from(Span::styled(
                "Cleanup incomplete — retry or log in again.",
                Style::default().fg(theme::DANGER),
            )),
        );
    }
    if !scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE) {
        lines.push(Line::from(Span::styled(
            "Gmail read-only — not granted",
            Style::default().fg(theme::WARNING),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No scopes recorded",
            Style::default().fg(theme::WARNING),
        )));
    }
    lines
}

fn scope_block_height(lines: &[Line<'_>], width: u16) -> u16 {
    let inner_width = usize::from(width.saturating_sub(4).max(1));
    let content_height = lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(inner_width))
        .sum::<usize>();
    u16::try_from(content_height.saturating_add(2))
        .unwrap_or(u16::MAX)
        .max(3)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_account_list(
    frame: &mut Frame<'_>,
    area: Rect,
    accounts: &[Account],
    selected: usize,
    target_subject: Option<&str>,
    mode: UiMode,
    focused: bool,
    scroll_offset: &mut usize,
) {
    let row_height = row_height(area, accounts.len(), mode);
    let list_inset = super::content_padding(area.width).saturating_add(1).min(3);
    let list_top = if mode != UiMode::Wide || accounts.is_empty() {
        2
    } else {
        3
    };
    let row_width = area
        .width
        .saturating_sub(2)
        .saturating_sub(list_inset.saturating_mul(2))
        .saturating_sub(2 + SCROLLBAR_RESERVED_WIDTH);
    let mut items: Vec<ListItem<'_>> = accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let name = account.email.as_str();
            let marker = if index == selected { ">" } else { " " };
            let target_marker = if target_subject == Some(account.subject.as_str()) {
                "◆"
            } else {
                " "
            };
            let (status_symbol, _, status_color) = status_presentation(account.connection_state);
            let card_padding: usize = if row_height >= 4 { 2 } else { 1 };
            if row_height >= 2 {
                let prefix = format!(
                    "{}{marker} {target_marker} {status_symbol}  {name}",
                    " ".repeat(card_padding)
                );
                let provider_gap = row_width
                    .saturating_sub(prefix.chars().count() as u16)
                    .saturating_sub((6 + card_padding) as u16)
                    as usize;
                let primary = Line::from(vec![
                    Span::styled(
                        format!("{}{marker} {target_marker} ", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                    Span::styled(status_symbol, Style::default().fg(status_color)),
                    Span::styled(format!("  {name}"), Style::default().fg(theme::TEXT)),
                    Span::styled(
                        format!("{:provider_gap$}Google{}", "", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                ]);
                let secondary = Line::from(vec![
                    Span::raw(" ".repeat(card_padding + 6)),
                    Span::styled(
                        format!(
                            "{}{} · {}",
                            if target_subject == Some(account.subject.as_str()) {
                                "MCP target · "
                            } else {
                                ""
                            },
                            status_label(account.connection_state),
                            scope_summary(account)
                        ),
                        Style::default().fg(theme::MUTED),
                    ),
                ]);
                if row_height >= 4 {
                    ListItem::new(vec![Line::from(""), primary, secondary, Line::from("")])
                } else {
                    ListItem::new(vec![primary, secondary])
                }
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}{marker} {target_marker} ", " ".repeat(card_padding)),
                        Style::default().fg(theme::PRIMARY),
                    ),
                    Span::styled(status_symbol, Style::default().fg(status_color)),
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
        let add_padding = if row_height >= 4 { 2 } else { 1 };
        let add_line = Line::from(vec![
            Span::styled(
                "+  ",
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::styled("Add another account", Style::default().fg(theme::PRIMARY)),
            Span::raw(" ".repeat(add_padding)),
        ]);
        items.push(ListItem::new(vec![
            Line::from(""),
            add_line,
            Line::from(""),
        ]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused {
            theme::PRIMARY_STRONG
        } else {
            theme::BORDER
        }))
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
    let scrollbar_area = Rect {
        x: list_area
            .x
            .saturating_add(list_area.width.saturating_sub(SCROLLBAR_TRACK_WIDTH)),
        y: list_area.y,
        width: SCROLLBAR_TRACK_WIDTH.min(list_area.width),
        height: list_area.height,
    };
    let list_content_area = Rect {
        width: list_area.width.saturating_sub(SCROLLBAR_RESERVED_WIDTH),
        ..list_area
    };
    let item_count = items.len();
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
    *state.offset_mut() = (*scroll_offset).min(item_count.saturating_sub(1));
    frame.render_stateful_widget(list, list_content_area, &mut state);
    *scroll_offset = state.offset();
    render_scrollbar(
        frame,
        scrollbar_area,
        item_count,
        list_viewport_rows(area, accounts.len(), mode),
        *scroll_offset,
        focused,
    );
}

pub(crate) fn list_viewport_rows(area: Rect, count: usize, mode: UiMode) -> usize {
    let inner_height = area.height.saturating_sub(2);
    let list_top = if mode != UiMode::Wide || count == 0 {
        2
    } else {
        3
    };
    let list_height = inner_height.saturating_sub(list_top);
    usize::from(list_height / row_height(area, count, mode).max(1)).max(1)
}

fn render_scrollbar(
    frame: &mut Frame<'_>,
    area: Rect,
    content_length: usize,
    viewport_length: usize,
    position: usize,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█")
        .track_style(Style::default().fg(theme::BORDER))
        .thumb_style(Style::default().fg(if focused {
            theme::PRIMARY_STRONG
        } else {
            theme::MUTED
        }));
    let mut state = ScrollbarState::new(content_length.max(1))
        .position(position.min(content_length.saturating_sub(1)))
        .viewport_content_length(viewport_length.max(1));
    frame.render_stateful_widget(scrollbar, area, &mut state);
}

pub(crate) fn render_account_details(
    frame: &mut Frame<'_>,
    area: Rect,
    account: Option<&Account>,
    target_subject: Option<&str>,
    mode: UiMode,
    focused: bool,
    details_scroll: &mut usize,
) {
    let Some(account) = account else {
        let block = panel(
            "",
            if focused {
                theme::PRIMARY_STRONG
            } else {
                theme::BORDER
            },
            area.width,
        );
        let inner = block.inner(area);
        let content = detail_content(inner, area.width);
        let empty_body = Rect {
            y: content.y.saturating_add(2),
            width: content.width.saturating_sub(SCROLLBAR_RESERVED_WIDTH),
            height: content.height.saturating_sub(2),
            ..content
        };
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
            Rect { ..empty_body },
        );
        render_scrollbar(
            frame,
            Rect {
                x: empty_body
                    .x
                    .saturating_add(empty_body.width)
                    .saturating_add(SCROLLBAR_PADDING),
                y: empty_body.y,
                width: SCROLLBAR_TRACK_WIDTH,
                height: empty_body.height,
            },
            usize::from(empty_body.height.max(1)),
            usize::from(empty_body.height.max(1)),
            0,
            focused,
        );
        return;
    };

    let block = panel(
        "",
        if focused {
            theme::PRIMARY_STRONG
        } else {
            theme::BORDER
        },
        area.width,
    );
    let inner = block.inner(area);
    let content = detail_content(inner, area.width);
    frame.render_widget(block, area);
    render_panel_header(frame, inner);
    let panel_header_height = 2;
    let identity_height = identity_height(mode);
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
            width: header_area
                .width
                .saturating_sub(CONNECTION_BADGE_WIDTH.saturating_add(2)),
            height: 1.min(header_area.height.saturating_sub(1)),
        },
    );
    let badge_area = connection_badge_area(area, mode);
    let (status_symbol, status_text, badge_color) = status_presentation(account.connection_state);
    frame.render_widget(
        Paragraph::new(format!("{status_symbol} {status_text}"))
            .style(
                Style::default()
                    .fg(badge_color)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(1))
                    .border_style(Style::default().fg(badge_color)),
            ),
        badge_area,
    );

    let gmail_access = match account.connection_state {
        ConnectionState::Disconnected => ("Gmail access", "Unavailable (revoked)", theme::WARNING),
        ConnectionState::Indeterminate => ("Gmail access", "Unknown", theme::DANGER),
        ConnectionState::Connected => match account.granted_scopes.as_deref() {
            None => ("Gmail access", "Unverified", theme::WARNING),
            Some(scopes) if scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE) => {
                ("Gmail access", "Granted", theme::SUCCESS)
            }
            Some(_) => ("Gmail access", "Not granted", theme::WARNING),
        },
    };
    let scope_evidence = if account.granted_scopes.is_some() {
        ("Scope evidence", "Confirmed", theme::SUCCESS)
    } else {
        ("Scope evidence", "Unverified", theme::WARNING)
    };
    let details = vec![
        ("Provider", "Google", theme::TEXT),
        gmail_access,
        scope_evidence,
        (
            "Credential",
            match account.connection_state {
                ConnectionState::Connected => "Protected (in OS keyring)",
                ConnectionState::Disconnected => "Not stored",
                ConnectionState::Indeterminate => "Cleanup incomplete",
            },
            match account.connection_state {
                ConnectionState::Connected => theme::SUCCESS,
                ConnectionState::Disconnected => theme::WARNING,
                ConnectionState::Indeterminate => theme::DANGER,
            },
        ),
        (
            "Keyring reference",
            account.token_key.as_deref().unwrap_or("Unavailable"),
            theme::TEXT,
        ),
    ];
    let detail_label_width = details
        .iter()
        .map(|(label, _, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let body = details_body_area(area, mode);
    let granted_scope_lines = scope_lines(account);
    let granted_scope_height = scope_block_height(&granted_scope_lines, body.width);
    let mut connection_lines = if mode == UiMode::Wide {
        vec![separator(body.width), Line::from("")]
    } else {
        Vec::new()
    };
    connection_lines.push(section_heading("Connection details"));
    connection_lines.push(Line::from(""));
    for (label, value, color) in details {
        connection_lines.push(detail_line_styled(label, value, color, detail_label_width));
    }
    connection_lines.push(Line::from(""));
    connection_lines.push(separator(body.width));
    let mut additional_lines = vec![
        separator(body.width),
        section_heading("Additional information"),
        Line::from(""),
    ];
    let additional_rows = [
        ("Account ID", account.id.as_str(), theme::TEXT),
        ("Type", "Personal", theme::TEXT),
        (
            "MCP target",
            if target_subject == Some(account.subject.as_str()) {
                "Selected"
            } else {
                "Not selected"
            },
            if target_subject == Some(account.subject.as_str()) {
                theme::SUCCESS
            } else {
                theme::MUTED
            },
        ),
        (
            "Status",
            status_label(account.connection_state),
            status_color(account.connection_state),
        ),
    ];
    let additional_label_width = additional_rows
        .iter()
        .map(|(label, _, _)| label.chars().count())
        .max()
        .unwrap_or(0)
        .max(detail_label_width);
    for (label, value, color) in additional_rows {
        additional_lines.push(detail_line_styled(
            label,
            value,
            color,
            additional_label_width,
        ));
    }
    additional_lines.push(Line::from(""));
    additional_lines.push(separator(body.width));
    let connection_height = connection_lines.len() as u16;
    let scope_heading_start = connection_height;
    let scope_heading_height = 2;
    let scope_start = scope_heading_start.saturating_add(scope_heading_height);
    let additional_start = scope_start
        .saturating_add(granted_scope_height)
        .saturating_add(1);
    let total_height = additional_start.saturating_add(additional_lines.len() as u16);
    *details_scroll = (*details_scroll).min(usize::from(total_height.saturating_sub(body.height)));
    render_scrolled_section(
        frame,
        body,
        *details_scroll,
        0,
        connection_height,
        Paragraph::new(connection_lines),
    );
    render_scrolled_section(
        frame,
        body,
        *details_scroll,
        scope_heading_start,
        scope_heading_height,
        Paragraph::new(vec![
            section_heading(scope_heading(account)),
            Line::from(""),
        ]),
    );
    render_scrolled_scope(
        frame,
        body,
        *details_scroll,
        scope_start,
        granted_scope_height,
        &granted_scope_lines,
    );
    render_scrolled_section(
        frame,
        body,
        *details_scroll,
        additional_start,
        additional_lines.len() as u16,
        Paragraph::new(additional_lines),
    );
    render_scrollbar(
        frame,
        Rect {
            x: body
                .x
                .saturating_add(body.width)
                .saturating_add(SCROLLBAR_PADDING),
            y: body.y,
            width: SCROLLBAR_TRACK_WIDTH,
            height: body.height,
        },
        usize::from(total_height.max(1)),
        usize::from(body.height.max(1)),
        *details_scroll,
        focused,
    );
}

pub(crate) fn details_viewport_rows(area: Rect, mode: UiMode) -> usize {
    usize::from(details_body_area(area, mode).height.max(1))
}

fn details_body_area(area: Rect, mode: UiMode) -> Rect {
    let block = panel("", theme::BORDER, area.width);
    let inner = block.inner(area);
    let content = detail_content(inner, area.width);
    let panel_header_height = 2;
    let identity_height = identity_height(mode);
    Rect {
        y: content
            .y
            .saturating_add(panel_header_height)
            .saturating_add(identity_height),
        height: content
            .height
            .saturating_sub(panel_header_height)
            .saturating_sub(identity_height),
        width: content.width.saturating_sub(SCROLLBAR_RESERVED_WIDTH),
        ..content
    }
}

fn render_scrolled_section<'a>(
    frame: &mut Frame<'_>,
    viewport: Rect,
    scroll: usize,
    start: u16,
    height: u16,
    widget: Paragraph<'a>,
) {
    let Some((visible_area, local_scroll, _)) = visible_section(viewport, scroll, start, height)
    else {
        return;
    };
    frame.render_widget(widget.scroll((local_scroll, 0)), visible_area);
}

fn render_scrolled_scope(
    frame: &mut Frame<'_>,
    viewport: Rect,
    scroll: usize,
    start: u16,
    height: u16,
    lines: &[Line<'static>],
) {
    let Some((visible_area, local_scroll, fully_visible)) =
        visible_section(viewport, scroll, start, height)
    else {
        return;
    };
    let mut paragraph = Paragraph::new(lines.to_vec())
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme::TEXT));
    let paragraph_scroll = if fully_visible {
        local_scroll
    } else {
        local_scroll.saturating_sub(1)
    };
    if fully_visible {
        paragraph = paragraph.block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BORDER))
                .padding(Padding::horizontal(1)),
        );
    } else {
        paragraph = paragraph.block(Block::default().padding(Padding::horizontal(2)));
    }
    frame.render_widget(paragraph.scroll((paragraph_scroll, 0)), visible_area);
}

fn visible_section(
    viewport: Rect,
    scroll: usize,
    start: u16,
    height: u16,
) -> Option<(Rect, u16, bool)> {
    let viewport_start = scroll as u16;
    let viewport_end = viewport_start.saturating_add(viewport.height);
    let section_end = start.saturating_add(height);
    let visible_start = start.max(viewport_start);
    let visible_end = section_end.min(viewport_end);
    if visible_start >= visible_end {
        return None;
    }
    let visible_area = Rect {
        x: viewport.x,
        y: viewport
            .y
            .saturating_add(visible_start.saturating_sub(viewport_start)),
        width: viewport.width,
        height: visible_end.saturating_sub(visible_start),
    };
    Some((
        visible_area,
        visible_start.saturating_sub(start),
        visible_start == start && visible_end == section_end,
    ))
}

fn status_color(state: ConnectionState) -> ratatui::style::Color {
    status_presentation(state).2
}

fn connection_badge_area(area: Rect, mode: UiMode) -> Rect {
    let block = panel("", theme::BORDER, area.width);
    let inner = block.inner(area);
    let content = detail_content(inner, area.width);
    let panel_header_height = 2;
    let identity_height = identity_height(mode);
    let header_area = Rect {
        y: content.y.saturating_add(panel_header_height),
        height: identity_height.min(content.height.saturating_sub(panel_header_height)),
        ..content
    };
    Rect {
        x: header_area
            .x
            .saturating_add(header_area.width.saturating_sub(CONNECTION_BADGE_WIDTH)),
        y: header_area.y,
        width: CONNECTION_BADGE_WIDTH.min(header_area.width),
        height: 3.min(header_area.height),
    }
}

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

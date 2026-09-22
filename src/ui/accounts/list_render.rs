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

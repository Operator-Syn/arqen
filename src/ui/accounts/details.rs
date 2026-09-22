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
                ConnectionState::Connected => "Protected credential available",
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

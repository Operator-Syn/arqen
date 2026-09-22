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

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

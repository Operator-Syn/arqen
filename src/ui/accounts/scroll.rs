// SPDX-License-Identifier: MPL-2.0
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

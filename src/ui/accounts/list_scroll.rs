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

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

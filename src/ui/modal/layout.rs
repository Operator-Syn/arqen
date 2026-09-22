fn action_width(action: &ModalAction) -> u16 {
    action.shortcut.len() as u16 + action.label.len() as u16 + 10
}

fn modal_padding(width: u16) -> u16 {
    super::content_padding(width).saturating_add(1).min(3)
}

fn action_gap(width: u16) -> u16 {
    super::content_padding(width).saturating_add(1).max(2)
}

fn wrapped_lines(text: &Text<'_>, width: u16) -> u16 {
    let width = usize::from(width.max(1));
    let lines = text
        .lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum::<usize>();
    u16::try_from(lines).unwrap_or(u16::MAX).max(1)
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn centered_horizontal(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area.y,
        width,
        height: area.height,
    }
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn tone_color(tone: ModalTone) -> ratatui::style::Color {
    match tone {
        ModalTone::Neutral => theme::PRIMARY_STRONG,
        ModalTone::Warning => theme::WARNING,
        ModalTone::Danger => theme::DANGER,
        ModalTone::Success => theme::SUCCESS,
    }
}

fn action_color(tone: ActionTone) -> ratatui::style::Color {
    match tone {
        ActionTone::Primary => theme::PRIMARY,
        ActionTone::Danger => theme::DANGER,
        ActionTone::Muted => theme::MUTED,
    }
}

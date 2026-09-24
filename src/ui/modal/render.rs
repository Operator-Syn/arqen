// SPDX-License-Identifier: MPL-2.0
impl Modal {
    pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, spec: &ModalSpec) {
        let layout = Self::layout(area, spec);
        frame.render_widget(Clear, layout.modal);
        let padding = Padding::horizontal(modal_padding(layout.modal.width));
        let block = Block::default()
            .title(
                Line::from(Span::styled(
                    format!(" {} ", spec.title),
                    Style::default().fg(tone_color(spec.tone)),
                ))
                .centered(),
            )
            .borders(Borders::ALL)
            .padding(padding)
            .border_style(Style::default().fg(tone_color(spec.tone)))
            .style(Style::default().bg(theme::SURFACE));
        frame.render_widget(block, layout.modal);
        frame.render_widget(
            Paragraph::new(spec.body.clone())
                .style(Style::default().fg(theme::TEXT))
                .wrap(Wrap { trim: true }),
            layout.body,
        );
        for (index, (action, rect)) in spec.actions.iter().zip(layout.actions).enumerate() {
            let focused = index == spec.focused_action;
            let color = if focused {
                action_color(action.tone)
            } else {
                theme::MUTED
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", action.shortcut),
                    Style::default().fg(color),
                ),
                Span::styled(
                    action.label.clone(),
                    Style::default()
                        .fg(color)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
            ]);
            frame.render_widget(
                Paragraph::new(line)
                    .alignment(ratatui::layout::Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(color)),
                    ),
                rect,
            );
        }
    }

    pub(crate) fn layout(area: Rect, spec: &ModalSpec) -> ModalLayout {
        let inset = super::content_padding(area.width);
        let inner_width = area.width.saturating_sub(inset.saturating_mul(2)).max(1);
        let border_width = 2u16.saturating_add(modal_padding(area.width).saturating_mul(2));
        let body_width = (spec
            .body
            .lines
            .iter()
            .map(|line| line.width())
            .max()
            .unwrap_or(1) as u16)
            // Long URLs and other unbroken tokens should wrap inside the
            // dialog instead of forcing it to the viewport edge.
            .min(area.width.saturating_mul(60) / 100);
        let title_width = spec.title.chars().count() as u16;
        let initial_gap = action_gap(area.width);
        let actions_width = spec
            .actions
            .iter()
            .map(action_width)
            .sum::<u16>()
            .saturating_add(
                initial_gap.saturating_mul(spec.actions.len().saturating_sub(1) as u16),
            );
        let desired_width = body_width
            .max(title_width)
            .max(actions_width)
            .saturating_add(border_width);
        let modal_width = desired_width
            .min(area.width.saturating_mul(72) / 100)
            .max(1);
        let modal_inner_width = modal_width
            .saturating_sub(border_width)
            .max(1)
            .min(inner_width);
        let horizontal = actions_width <= modal_inner_width;
        let action_rows = if horizontal {
            1
        } else {
            spec.actions.len() as u16
        };
        let modal_pad = modal_padding(modal_width);
        let body_width = modal_width
            .saturating_sub(2 + modal_pad.saturating_mul(2))
            .max(1);
        let body_lines = wrapped_lines(&spec.body, body_width);
        let breathing: u16 = if body_lines > 1 { 2 } else { 1 };
        let action_spacing: u16 = if body_lines > 1 { 2 } else { 1 };
        let desired_height = body_lines
            .saturating_add(action_rows.saturating_mul(3))
            .saturating_add(breathing.saturating_mul(2))
            .saturating_add(action_spacing)
            .saturating_add(2);
        let modal_height = desired_height
            .min(area.height.saturating_mul(62) / 100)
            .max(1);
        let modal = centered(area, modal_width, modal_height);
        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(modal_padding(modal.width)));
        let body_and_actions = block.inner(modal);
        let body_height = body_and_actions
            .height
            .saturating_sub(action_rows.saturating_mul(3))
            .saturating_sub(breathing.saturating_mul(2))
            .saturating_sub(action_spacing);
        let breathing: u16 = if body_lines > 1 { 2 } else { 1 };
        let sections = Layout::vertical([
            Constraint::Length(breathing),
            Constraint::Min(body_height.max(1)),
            Constraint::Length(action_spacing),
            Constraint::Length(action_rows.saturating_mul(3)),
            Constraint::Length(breathing),
        ])
        .split(body_and_actions);
        let actions = if horizontal {
            let constraints = spec
                .actions
                .iter()
                .map(|action| Constraint::Length(action_width(action)))
                .collect::<Vec<_>>();
            let group_width = spec
                .actions
                .iter()
                .map(action_width)
                .sum::<u16>()
                .saturating_add(
                    action_gap(modal.width)
                        .saturating_mul(spec.actions.len().saturating_sub(1) as u16),
                );
            let group = centered_horizontal(sections[3], group_width);
            Layout::horizontal(constraints)
                .spacing(action_gap(modal.width))
                .split(group)
                .to_vec()
        } else {
            Layout::vertical(
                spec.actions
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            )
            .split(sections[3])
            .iter()
            .zip(spec.actions.iter())
            .map(|(rect, action)| centered_horizontal(*rect, action_width(action)))
            .collect()
        };
        ModalLayout {
            modal,
            body: sections[1],
            actions,
        }
    }

    pub(crate) fn hit_test(
        area: Rect,
        spec: &ModalSpec,
        column: u16,
        row: u16,
    ) -> Option<ModalActionId> {
        let layout = Self::layout(area, spec);
        spec.actions
            .iter()
            .zip(layout.actions)
            .find(|(_, rect)| contains(*rect, column, row))
            .map(|(action, _)| action.id)
    }
}

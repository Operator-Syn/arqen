#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ModalSpec {
        ModalSpec {
            title: "Confirm quit".into(),
            tone: ModalTone::Warning,
            body: Text::from("Exit Arqen?\n\nNo account data will be changed."),
            actions: vec![
                ModalAction {
                    id: ModalActionId::Confirm,
                    label: "QUIT".into(),
                    shortcut: "Enter/y".into(),
                    tone: ActionTone::Danger,
                },
                ModalAction {
                    id: ModalActionId::Cancel,
                    label: "CANCEL".into(),
                    shortcut: "Esc/n".into(),
                    tone: ActionTone::Primary,
                },
            ],
            focused_action: 0,
        }
    }

    #[test]
    fn actions_fit_horizontally_when_space_allows() {
        let layout = Modal::layout(Rect::new(0, 0, 120, 32), &spec());
        assert_eq!(layout.actions.len(), 2);
        assert_eq!(layout.actions[0].y, layout.actions[1].y);
        let group_start = layout.actions[0].x;
        let group_end = layout.actions[1].x + layout.actions[1].width;
        assert!((group_start + group_end) >= 118 && (group_start + group_end) <= 122);
    }

    #[test]
    fn actions_stack_on_compact_space() {
        let layout = Modal::layout(Rect::new(0, 0, 40, 14), &spec());
        assert_eq!(layout.actions.len(), 2);
        assert!(layout.actions[1].y > layout.actions[0].y);
    }

    #[test]
    fn hit_test_returns_action_id() {
        let viewport = Rect::new(0, 0, 120, 32);
        let layout = Modal::layout(viewport, &spec());
        assert_eq!(
            Modal::hit_test(
                viewport,
                &spec(),
                layout.actions[0].x + 1,
                layout.actions[0].y + 1,
            ),
            Some(ModalActionId::Confirm)
        );
    }
}

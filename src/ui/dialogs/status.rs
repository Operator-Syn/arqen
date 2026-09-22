pub(crate) fn error_spec(message: &str, compact: bool) -> ModalSpec {
    ModalSpec {
        title: error_title(message).into(),
        tone: ModalTone::Danger,
        body: Text::from(message.to_owned()),
        actions: vec![action(
            ModalActionId::Close,
            "CLOSE",
            if compact { "Esc" } else { "Enter/Esc" },
            ActionTone::Primary,
        )],
        focused_action: 0,
    }
}

fn error_title(message: &str) -> &'static str {
    if message.starts_with("Reauthentication not completed") {
        "Reauthentication not completed"
    } else if message.starts_with("Reconnection not completed") {
        "Reconnection not completed"
    } else if message.starts_with("Unable to disconnect") {
        "Disconnect not completed"
    } else {
        "Login error"
    }
}

fn action(id: ModalActionId, label: &str, shortcut: &str, tone: ActionTone) -> ModalAction {
    ModalAction {
        id,
        label: label.into(),
        shortcut: shortcut.into(),
        tone,
    }
}

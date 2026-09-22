pub(crate) fn confirm_quit_spec(compact: bool) -> ModalSpec {
    ModalSpec {
        title: "Confirm quit".into(),
        tone: ModalTone::Warning,
        body: if compact {
            Text::from(vec![Line::from(vec![
                Span::styled(
                    "[!]",
                    ratatui::style::Style::default().fg(super::theme::WARNING),
                ),
                Span::raw(" Exit Arqen?"),
            ])])
        } else {
            Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        "[!]",
                        ratatui::style::Style::default().fg(super::theme::WARNING),
                    ),
                    Span::raw(" Exit Arqen?"),
                ]),
                Line::from(""),
                Line::from("No account data will be changed.").alignment(Alignment::Center),
                Line::from("Any in-progress login will be cancelled.").alignment(Alignment::Center),
            ])
        },
        actions: vec![
            action(
                ModalActionId::Confirm,
                "QUIT",
                "Enter/y",
                ActionTone::Danger,
            ),
            action(
                ModalActionId::Cancel,
                "CANCEL",
                "Esc/n",
                ActionTone::Primary,
            ),
        ],
        focused_action: 0,
    }
}

pub(crate) fn disconnect_spec(email: &str, retry: bool, compact: bool) -> ModalSpec {
    let title = if retry {
        "Retry account disconnect"
    } else {
        "Disconnect Google account"
    };
    let body = if compact {
        Text::from(vec![Line::from(if retry {
            "Retry revocation and local credential cleanup?"
        } else {
            "Revoke Google access and remove the local credential?"
        })])
    } else {
        Text::from(vec![
            Line::from(if retry {
                "Retry cleanup for this Google account?"
            } else {
                "Disconnect this Google account?"
            }),
            Line::from(email.to_owned()).alignment(Alignment::Center),
            Line::from(""),
            Line::from(if retry {
                "Arqen will retry provider revocation and protected credential cleanup."
            } else {
                "Google access will be revoked and the local refresh token removed."
            }),
            Line::from("The identity stays in Arqen so it can be connected again.")
                .alignment(Alignment::Center),
        ])
    };
    ModalSpec {
        title: title.into(),
        tone: if retry {
            ModalTone::Warning
        } else {
            ModalTone::Danger
        },
        body,
        actions: vec![
            action(
                ModalActionId::Confirm,
                if retry { "RETRY" } else { "DISCONNECT" },
                "Enter/y",
                if retry {
                    ActionTone::Primary
                } else {
                    ActionTone::Danger
                },
            ),
            action(ModalActionId::Cancel, "CANCEL", "Esc/n", ActionTone::Muted),
        ],
        focused_action: 0,
    }
}

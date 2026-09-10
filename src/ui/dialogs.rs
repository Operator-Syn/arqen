use super::modal::{ActionTone, Modal, ModalAction, ModalActionId, ModalSpec, ModalTone};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    prelude::{Line, Span, Text},
};

pub(crate) fn render_authorization(
    frame: &mut Frame<'_>,
    area: Rect,
    url: &str,
    compact: bool,
    manual_fallback: bool,
    reauthenticate: bool,
    reconnect: bool,
) {
    let spec = authorization_spec(url, compact, manual_fallback, reauthenticate, reconnect);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_redirect(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: Option<&str>,
    input: &str,
    compact: bool,
) {
    let spec = redirect_spec(notice, input, compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_error(frame: &mut Frame<'_>, area: Rect, message: &str, compact: bool) {
    let spec = error_spec(message, compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_confirm_quit(frame: &mut Frame<'_>, area: Rect, compact: bool) {
    let spec = confirm_quit_spec(compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_disconnect(
    frame: &mut Frame<'_>,
    area: Rect,
    email: &str,
    retry: bool,
    compact: bool,
) {
    let spec = disconnect_spec(email, retry, compact);
    Modal::render(frame, area, &spec);
}

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
                "Arqen will retry provider revocation and keyring cleanup."
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

pub(crate) fn authorization_spec(
    url: &str,
    compact: bool,
    manual_fallback: bool,
    reauthenticate: bool,
    reconnect: bool,
) -> ModalSpec {
    let title = if reconnect {
        "Reconnect Google account"
    } else if reauthenticate {
        "Reauthenticate Google account"
    } else {
        "Connect Google account"
    };
    let body = if compact {
        Text::from(vec![
            Line::from(if reconnect {
                "Open the URL to reconnect this account."
            } else if reauthenticate {
                "Open the URL to refresh this account's grant."
            } else {
                "Open the URL and approve access."
            }),
            Line::from(Span::styled(
                url.to_owned(),
                ratatui::style::Style::default().fg(super::theme::PRIMARY),
            )),
        ])
    } else {
        Text::from(vec![
            Line::from("Open this authorization URL in your browser:"),
            Line::from(Span::styled(
                url.to_owned(),
                ratatui::style::Style::default().fg(super::theme::PRIMARY),
            )),
            Line::from(""),
            Line::from(if reconnect || reauthenticate {
                "1. Choose the same Google account to refresh its grant."
            } else {
                "1. Choose the Google account to connect."
            }),
            Line::from(
                "2. Follow the Google login procedure and approve only the permissions you want to apply.",
            ),
            if manual_fallback {
                Line::from(
                    "Automatic callback unavailable; press Enter to use manual redirect input.",
                )
            } else {
                Line::from("")
            },
        ])
    };
    ModalSpec {
        title: title.into(),
        tone: ModalTone::Neutral,
        body,
        actions: {
            let mut actions = vec![
                action(ModalActionId::Copy, "COPY", "c", ActionTone::Primary),
                action(ModalActionId::Open, "OPEN", "o", ActionTone::Primary),
            ];
            if manual_fallback {
                actions.push(action(
                    ModalActionId::Continue,
                    "CONTINUE",
                    "Enter",
                    ActionTone::Primary,
                ));
            }
            actions.push(action(
                ModalActionId::Cancel,
                "CANCEL",
                "Esc",
                ActionTone::Muted,
            ));
            actions
        },
        focused_action: if manual_fallback { 2 } else { 1 },
    }
}

pub(crate) fn redirect_spec(notice: Option<&str>, input: &str, _compact: bool) -> ModalSpec {
    let message = notice.unwrap_or("Paste the complete browser redirect URL:");
    ModalSpec {
        title: "Finish connection".into(),
        tone: ModalTone::Neutral,
        body: Text::from(vec![
            Line::from(message.to_owned()),
            Line::from(""),
            Line::from(format!("{input}|")),
        ]),
        actions: vec![
            action(
                ModalActionId::Continue,
                "SUBMIT",
                "Enter",
                ActionTone::Primary,
            ),
            action(ModalActionId::Cancel, "CANCEL", "Esc", ActionTone::Muted),
        ],
        focused_action: 0,
    }
}

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

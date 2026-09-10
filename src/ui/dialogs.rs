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
) {
    let spec = authorization_spec(url, compact, manual_fallback);
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

pub(crate) fn authorization_spec(url: &str, compact: bool, manual_fallback: bool) -> ModalSpec {
    let body = if compact {
        Text::from(vec![
            Line::from("Open the URL and approve access."),
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
            Line::from("1. Choose the Google account to connect."),
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
        title: "Connect Google account".into(),
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
        title: "Login error".into(),
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

fn action(id: ModalActionId, label: &str, shortcut: &str, tone: ActionTone) -> ModalAction {
    ModalAction {
        id,
        label: label.into(),
        shortcut: shortcut.into(),
        tone,
    }
}

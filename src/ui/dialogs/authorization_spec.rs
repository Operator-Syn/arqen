pub(crate) fn authorization_spec(
    url: &str,
    compact: bool,
    manual_fallback: bool,
    remote: bool,
    reauthenticate: bool,
    reconnect: bool,
) -> ModalSpec {
    let login_helper_enabled = crate::tui::LOGIN_HELPER_ENABLED && !manual_fallback && !remote;
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
                if remote {
                    "Open the URL in your local browser while the SSH tunnel remains connected."
                } else if manual_fallback {
                    "Open the URL to reconnect this account."
                } else if login_helper_enabled {
                    "Open the login helper to reconnect this account."
                } else {
                    "Open the Google sign-in URL to reconnect this account."
                }
            } else if reauthenticate {
                if remote {
                    "Open the URL in your local browser while the SSH tunnel remains connected."
                } else if manual_fallback {
                    "Open the URL to refresh this account's grant."
                } else if login_helper_enabled {
                    "Open the login helper to refresh this account's grant."
                } else {
                    "Open the Google sign-in URL to refresh this account's grant."
                }
            } else {
                if remote {
                    "Open the URL in your local browser while the SSH tunnel remains connected."
                } else if manual_fallback {
                    "Open the URL and approve access."
                } else if login_helper_enabled {
                    "Open the login helper and approve access."
                } else {
                    "Open the Google sign-in URL and approve access."
                }
            }),
            Line::from(Span::styled(
                url.to_owned(),
                ratatui::style::Style::default().fg(super::theme::PRIMARY),
            )),
        ])
    } else {
        Text::from(vec![
            Line::from(if remote {
                "Open this URL in your local browser; the SSH tunnel returns the callback here:"
            } else if manual_fallback {
                "Open this authorization URL in your browser:"
            } else if login_helper_enabled {
                "Use [o] to open the login helper, or [c] to copy this URL:"
            } else {
                "Use [o] to open a dedicated Google sign-in window, or [c] to copy this URL:"
            }),
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
            if remote {
                Line::from("Keep the SSH tunnel connected until Arqen confirms the account.")
            } else if manual_fallback {
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
            let mut actions = vec![action(
                ModalActionId::Copy,
                "COPY",
                "c",
                ActionTone::Primary,
            )];
            if !remote {
                actions.push(action(
                    ModalActionId::Open,
                    "OPEN",
                    "o",
                    ActionTone::Primary,
                ));
            }
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
        focused_action: if manual_fallback {
            actions_len_for_focus(remote)
        } else if remote {
            0
        } else {
            1
        },
    }
}

fn actions_len_for_focus(remote: bool) -> usize {
    if remote { 1 } else { 2 }
}

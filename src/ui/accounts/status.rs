fn status_presentation(
    state: ConnectionState,
) -> (&'static str, &'static str, ratatui::style::Color) {
    match state {
        ConnectionState::Connected => ("●", "CONNECTED", theme::SUCCESS),
        ConnectionState::Disconnected => ("○", "DISCONNECTED", theme::WARNING),
        ConnectionState::Indeterminate => ("!", "UNKNOWN", theme::DANGER),
    }
}

pub(crate) fn status_label(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Connected => "Connected",
        ConnectionState::Disconnected => "Disconnected",
        ConnectionState::Indeterminate => "Connection unknown",
    }
}

pub(crate) fn scope_summary(account: &Account) -> String {
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return "Scopes unverified".into();
    };
    let mut labels = Vec::new();
    let mut unknown = 0usize;
    for scope in scopes {
        if let Some(label) = friendly_scope(scope) {
            labels.push(label.to_owned());
        } else {
            unknown += 1;
        }
    }
    labels.sort();
    labels.dedup();
    if unknown > 0 {
        if labels.is_empty() {
            format!("{unknown} unrecognized scope(s)")
        } else {
            format!("{} + {unknown} other", labels.join(" · "))
        }
    } else if labels.is_empty() {
        "No scopes recorded".into()
    } else {
        labels.join(" · ")
    }
}

fn friendly_scope(scope: &str) -> Option<&'static str> {
    match scope {
        "openid" => Some("OpenID identity"),
        "email" | EMAIL_SCOPE => Some("Email address"),
        "profile" | PROFILE_SCOPE => Some("Basic profile"),
        GMAIL_READONLY_SCOPE => Some("Gmail read-only"),
        _ => None,
    }
}

fn scope_lines(account: &Account) -> Vec<Line<'static>> {
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return vec![Line::from(Span::styled(
            "Scopes unverified — reauthenticate to record Google's grant.",
            Style::default().fg(theme::WARNING),
        ))];
    };

    let mut lines: Vec<Line<'static>> = scopes
        .iter()
        .map(|scope| {
            let text = friendly_scope(scope)
                .map(|label| format!("{label} — {scope}"))
                .unwrap_or_else(|| scope.clone());
            Line::from(Span::styled(text, Style::default().fg(theme::TEXT)))
        })
        .collect();
    if account.connection_state == ConnectionState::Disconnected {
        lines.insert(
            0,
            Line::from(Span::styled(
                "Provider grant revoked — reconnect to restore access.",
                Style::default().fg(theme::WARNING),
            )),
        );
    } else if account.connection_state == ConnectionState::Indeterminate {
        lines.insert(
            0,
            Line::from(Span::styled(
                "Cleanup incomplete — retry or log in again.",
                Style::default().fg(theme::DANGER),
            )),
        );
    }
    if !scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE) {
        lines.push(Line::from(Span::styled(
            "Gmail read-only — not granted",
            Style::default().fg(theme::WARNING),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No scopes recorded",
            Style::default().fg(theme::WARNING),
        )));
    }
    lines
}

fn scope_block_height(lines: &[Line<'_>], width: u16) -> u16 {
    let inner_width = usize::from(width.saturating_sub(4).max(1));
    let content_height = lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(inner_width))
        .sum::<usize>();
    u16::try_from(content_height.saturating_add(2))
        .unwrap_or(u16::MAX)
        .max(3)
}

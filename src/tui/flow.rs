fn browser_target(callback: Option<&crate::callback::CallbackServer>, authorization_url: &str) -> String {
    browser_target_with_mode(LOGIN_HELPER_ENABLED, callback, authorization_url)
}

fn browser_target_with_mode(
    login_helper_enabled: bool,
    callback: Option<&crate::callback::CallbackServer>,
    authorization_url: &str,
) -> String {
    if login_helper_enabled {
        callback
            .map(|callback| callback.launcher_uri().to_owned())
            .unwrap_or_else(|| authorization_url.to_owned())
    } else {
        authorization_url.to_owned()
    }
}

fn login_error_context(intent: &LoginIntent) -> &'static str {
    match intent {
        LoginIntent::Add => "Login failed",
        LoginIntent::Reauthenticate { .. } => "Reauthentication not completed",
        LoginIntent::Reconnect { .. } => "Reconnection not completed",
    }
}

fn mcp_target_ineligibility(account: &Account) -> Option<&'static str> {
    if account.connection_state != ConnectionState::Connected {
        return Some("the account is not connected");
    }
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return Some("Google's granted scopes are unverified");
    };
    if !scopes.iter().any(|scope| scope == GMAIL_READONLY_SCOPE) {
        return Some("the recorded grant does not include Gmail read-only access");
    }
    if account.token_key.is_none() {
        return Some("the account has no protected credential reference");
    }
    None
}

fn friendly_login_error(intent: &LoginIntent, error: &anyhow::Error) -> String {
    let detail = format!("{error:#}");
    if !detail.contains(arqen::auth::SUBJECT_MISMATCH_MESSAGE) {
        return detail;
    }
    match intent {
        LoginIntent::Reauthenticate { .. } => {
            "Google returned a different account than the one selected for reauthentication.\n\nNo account data or credentials were changed. Close this message and sign in with the same Google account to try again.".into()
        }
        LoginIntent::Reconnect { .. } => {
            "Google returned a different account than the one on this card.\n\nNo account data or credentials were changed. Close this message and sign in with the card's Google account to reconnect it.".into()
        }
        LoginIntent::Add => detail,
    }
}

fn copy_to_clipboard(clipboard: &mut Option<arboard::Clipboard>, text: &str) -> Result<()> {
    if clipboard.is_none() {
        match arboard::Clipboard::new() {
            Ok(value) => *clipboard = Some(value),
            Err(error) => return copy_with_fallback(text, anyhow::anyhow!(error)),
        }
    }
    if let Some(value) = clipboard.as_mut() {
        match value
            .set_text(text)
            .context("write authorization URL to clipboard")
        {
            Ok(()) => return Ok(()),
            Err(error) => {
                *clipboard = None;
                return copy_with_fallback(text, error);
            }
        }
    }
    anyhow::bail!("clipboard provider was unavailable")
}

fn copy_with_fallback(text: &str, primary_error: anyhow::Error) -> Result<()> {
    let mut failures = Vec::new();
    for (program, args) in [
        ("wl-copy", Vec::new()),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ] {
        let mut child = match std::process::Command::new(program)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                failures.push(format!("{program}: {error}"));
                continue;
            }
        };
        let write_result = child
            .stdin
            .take()
            .context("open clipboard fallback input")
            .and_then(|mut input| {
                input
                    .write_all(text.as_bytes())
                    .context("write clipboard fallback input")
            });
        if let Err(error) = write_result {
            failures.push(format!("{program}: {error}"));
            let _ = child.kill();
            continue;
        }
        if let Some(status) = child.try_wait().context("check clipboard fallback")? {
            if status.success() {
                return Ok(());
            }
            failures.push(format!("{program}: exited with {status}"));
        } else {
            return Ok(());
        }
    }
    anyhow::bail!(
        "{primary_error:#}; clipboard fallbacks unavailable ({})",
        failures.join("; ")
    )
}

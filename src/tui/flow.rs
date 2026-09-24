// SPDX-License-Identifier: MPL-2.0
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
    let chain = error
        .chain()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let contains = |text: &str| chain.iter().any(|part| part.contains(text));
    let openbao_failure = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<arqen::secrets::OpenBaoFailure>())
        .copied();

    if contains(arqen::auth::SUBJECT_MISMATCH_MESSAGE) {
        return match intent {
            LoginIntent::Reauthenticate { .. } => "Google returned a different account than the one selected for reauthentication.\n\nNo account data or credentials were changed. Sign in with the same Google account selected in Arqen.".into(),
            LoginIntent::Reconnect { .. } => "Google returned a different account than the one on this card.\n\nNo account data or credentials were changed. Sign in with the Google account shown on the card.".into(),
            LoginIntent::Add => "Google returned an account profile Arqen could not verify. No credentials or account details were saved. Retry the login.".into(),
        };
    }

    match openbao_failure {
        Some(arqen::secrets::OpenBaoFailure::AppRoleRejected) => {
            return "Google authorization completed, but OpenBao rejected Arqen's service credentials. Arqen could not save the protected refresh token or the account's granted scopes.\n\nRun `make docker-up` to check and repair the local OpenBao service credentials, then retry. Google's consent may already have been recorded.".into();
        }
        Some(arqen::secrets::OpenBaoFailure::Unavailable) => {
            return "Google authorization completed, but Arqen could not reach OpenBao to save the protected refresh token. The account's granted scopes were not saved. Check that the Docker services are running, run `make docker-up`, and retry.".into();
        }
        Some(arqen::secrets::OpenBaoFailure::CredentialWriteFailed(403)) => {
            return "Google authorization completed and Arqen authenticated to OpenBao, but OpenBao denied the protected refresh-token write. Arqen did not save the account's granted scopes. Ask the local OpenBao operator to check the Arqen service policy, then retry.".into();
        }
        Some(arqen::secrets::OpenBaoFailure::CredentialWriteFailed(_)) => {
            return "Google authorization completed, but Arqen could not confirm that OpenBao saved the protected refresh token. The account's granted scopes were not saved. Check OpenBao service health, then retry when it is available.".into();
        }
        Some(arqen::secrets::OpenBaoFailure::LoginFailed(_)) => {
            return "Google authorization completed, but OpenBao did not accept Arqen's service-login request. Arqen did not save the protected refresh token or the account's granted scopes. Check the `make docker-up` output and the local OpenBao AppRole configuration before retrying.".into();
        }
        Some(arqen::secrets::OpenBaoFailure::InvalidResponse) => {
            return "Google authorization completed, but OpenBao returned an invalid response while Arqen was saving the protected refresh token. The account's granted scopes were not saved. Check the Docker services, run `make docker-up`, then retry.".into();
        }
        Some(
            arqen::secrets::OpenBaoFailure::CredentialReadFailed(_)
            | arqen::secrets::OpenBaoFailure::CredentialDeleteFailed(_),
        )
        | None => {}
    }
    if contains("save Google account metadata in SQLite") {
        let retry_action = match intent {
            LoginIntent::Add => "then retry adding the account",
            LoginIntent::Reauthenticate { .. } => "then retry reauthentication for the same account",
            LoginIntent::Reconnect { .. } => "then retry reconnecting the account",
        };
        return format!("Arqen saved the protected refresh token, but could not save the account details and granted scopes to its local database. The account may still show its previous connection and scope state. Check local storage, {retry_action}.");
    }
    if contains("Google did not return a refresh token") {
        return "Google did not return a refresh token, so Arqen could not save the protected credential or update the account's granted scopes. Retry authorization and approve the requested access.".into();
    }
    if contains("Google rejected the authorization-code exchange") {
        return "Google did not complete Arqen's authorization-code exchange. No refresh token or account scope changes were saved. Start a new login and complete the Google consent step.".into();
    }
    if contains("Google authorization failed:") {
        return "Google did not complete the authorization step. Arqen did not save a refresh token or account scope changes. Start a new login and complete the Google consent step.".into();
    }
    if contains("OAuth state mismatch") {
        return "Arqen could not verify this Google authorization response. No refresh token or account scope changes were saved. Close this message and start a new login from Arqen.".into();
    }
    if contains("send authorization-code exchange to Google") {
        return "Arqen could not reach Google to complete the authorization-code exchange. No refresh token or account scope changes were saved. Check the network connection and retry.".into();
    }
    if contains("Google rejected the profile request") {
        return "Google authorized the request, but Arqen could not verify the account profile. No refresh token or account scope changes were saved. Retry the login.".into();
    }
    if contains("request Google account profile") {
        return "Google authorized the request, but Arqen could not reach Google to verify the account profile. No refresh token or account scope changes were saved. Check the network connection and retry.".into();
    }

    let retry_action = match intent {
        LoginIntent::Add => "After Arqen is ready, retry adding the Google account.",
        LoginIntent::Reauthenticate { .. } => {
            "After Arqen is ready, retry reauthentication for the same selected Google account."
        }
        LoginIntent::Reconnect { .. } => {
            "After Arqen is ready, retry reconnecting the Google account on this card."
        }
    };
    format!("Arqen could not complete this login. It did not save the account's granted scopes. {retry_action}")
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

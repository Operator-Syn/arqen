fn handle_list_emails(
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database is unavailable",
            );
        }
    };
    let target_subject = match store.mcp_configuration() {
        Ok(configuration) => match configuration.target_google_subject {
            Some(subject) => subject,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetNotConfigured,
                    "select an eligible MCP target account in Arqen first",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database configuration is unavailable",
            );
        }
    };
    let account = match store.list_accounts() {
        Ok(accounts) => match accounts
            .into_iter()
            .find(|account| account.subject == target_subject)
        {
            Some(account) => account,
            None => {
                return BrokerResponse::error(
                    BrokerErrorCode::TargetUnavailable,
                    "the configured MCP target account is no longer available",
                );
            }
        },
        Err(_) => {
            return BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database could not be read",
            );
        }
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    let result = list_with_refresh(&account, request, state);
    match result {
        Ok(result) => BrokerResponse::Ok { result },
        Err(error) if is_invalid_grant(&error) || is_missing_refresh_token(&error) => {
            BrokerResponse::error(
                BrokerErrorCode::ReauthenticationRequired,
                "reauthenticate the selected MCP target account in Arqen",
            )
        }
        Err(error) => map_gmail_error(&error),
    }
}

fn target_ineligibility(account: &Account) -> Option<&'static str> {
    if account.connection_state != ConnectionState::Connected {
        return Some("the configured MCP target account is not connected");
    }
    let Some(scopes) = account.granted_scopes.as_deref() else {
        return Some("the configured MCP target has unverified Google scopes");
    };
    if !scopes
        .iter()
        .any(|scope| scope == crate::GMAIL_READONLY_SCOPE)
    {
        return Some("the configured MCP target has no recorded Gmail read-only grant");
    }
    if account.token_key.is_none() {
        return Some("the configured MCP target has no protected credential reference");
    }
    None
}

fn list_with_refresh(
    account: &Account,
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> Result<EmailListResponse> {
    let api = GmailApi::new()?;
    let token = cached_or_refresh_token(account, state)?;
    match api.list_emails(&token, &account.email, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            invalidate_token(&account.subject, state);
            let token = refresh_token(account, state)?;
            api.list_emails(&token, &account.email, request)
        }
        Err(error) => Err(error),
    }
}

fn cached_or_refresh_token(account: &Account, state: &BrokerState) -> Result<String> {
    if let Some(token) = state
        .access_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("access-token cache is unavailable"))?
        .get(&account.subject)
        .filter(|token| token.expires_at > Instant::now())
        .map(|token| token.value.clone())
    {
        return Ok(token);
    }
    refresh_token(account, state)
}

fn refresh_token(account: &Account, state: &BrokerState) -> Result<String> {
    let refreshed = refresh_google_access_token(
        &state.credentials_path,
        account.token_key.as_deref(),
        &account.subject,
    )?;
    let lifetime = refreshed
        .expires_in
        .unwrap_or(DEFAULT_ACCESS_TOKEN_SECONDS)
        .saturating_sub(ACCESS_TOKEN_SKEW_SECONDS)
        .max(30);
    let value = refreshed.value;
    state
        .access_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("access-token cache is unavailable"))?
        .insert(
            account.subject.clone(),
            CachedAccessToken {
                value: value.clone(),
                expires_at: Instant::now() + Duration::from_secs(lifetime),
            },
        );
    Ok(value)
}

fn invalidate_token(subject: &str, state: &BrokerState) {
    if let Ok(mut cache) = state.access_tokens.lock() {
        cache.remove(subject);
    }
}

fn map_gmail_error(error: &anyhow::Error) -> BrokerResponse {
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            429 => {
                return BrokerResponse::error(
                    BrokerErrorCode::GmailRateLimited,
                    "Gmail is rate limiting requests; try again shortly",
                );
            }
            _ => {}
        }
        return BrokerResponse::error(
            BrokerErrorCode::GmailUnavailable,
            "Gmail could not complete the mail-list request",
        );
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not complete the mail-list request",
    )
}

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
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    match list_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::Ok { result },
        Err(ListEmailsFailure::Credential(error)) => map_credential_error(&error),
        Err(ListEmailsFailure::Gmail(error)) => map_gmail_error(&error),
    }
}

fn handle_read_email(request: crate::gmail::ReadEmailRequest, state: &BrokerState) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    match read_with_refresh(&account, request, state) {
        Ok(result) => BrokerResponse::ReadEmail { result },
        Err(ReadEmailFailure::Credential(error)) => map_credential_error(&error),
        Err(ReadEmailFailure::Gmail(error)) => map_read_email_error(&error),
    }
}

fn handle_list_labels(state: &BrokerState) -> BrokerResponse {
    let store = match AccountStore::open(&state.database_path) {
        Ok(store) => store,
        Err(_) => return account_database_unavailable(),
    };
    let account = match selected_target_account(&store) {
        Ok(account) => account,
        Err(error) => return error.into_response(),
    };
    if let Some(reason) = target_ineligibility(&account) {
        return BrokerResponse::error(BrokerErrorCode::TargetUnavailable, reason);
    }
    match list_labels_with_refresh(&account, state) {
        Ok(result) => BrokerResponse::Labels { result },
        Err(ListLabelsFailure::Credential(error)) => map_credential_error(&error),
        Err(ListLabelsFailure::Gmail(error)) => map_gmail_error(&error),
    }
}

fn account_database_unavailable() -> BrokerResponse {
    BrokerResponse::error(
        BrokerErrorCode::Internal,
        "the account database is unavailable",
    )
}

#[derive(Debug, Clone, Copy)]
enum TargetAccountFailure {
    NotConfigured,
    ConfigurationUnavailable,
    AccountUnavailable,
    AccountsUnavailable,
}

impl TargetAccountFailure {
    fn into_response(self) -> BrokerResponse {
        match self {
            Self::NotConfigured => BrokerResponse::error(
                BrokerErrorCode::TargetNotConfigured,
                "select an eligible MCP target account in Arqen first",
            ),
            Self::ConfigurationUnavailable => BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database configuration is unavailable",
            ),
            Self::AccountUnavailable => BrokerResponse::error(
                BrokerErrorCode::TargetUnavailable,
                "the configured MCP target account is no longer available",
            ),
            Self::AccountsUnavailable => BrokerResponse::error(
                BrokerErrorCode::Internal,
                "the account database could not be read",
            ),
        }
    }
}

fn selected_target_account(
    store: &AccountStore,
) -> std::result::Result<Account, TargetAccountFailure> {
    let target_subject = match store.mcp_configuration() {
        Ok(configuration) => match configuration.target_google_subject {
            Some(subject) => subject,
            None => return Err(TargetAccountFailure::NotConfigured),
        },
        Err(_) => return Err(TargetAccountFailure::ConfigurationUnavailable),
    };
    match store.list_accounts() {
        Ok(accounts) => accounts
            .into_iter()
            .find(|account| account.subject == target_subject)
            .ok_or(TargetAccountFailure::AccountUnavailable),
        Err(_) => Err(TargetAccountFailure::AccountsUnavailable),
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

#[derive(Debug)]
enum ListEmailsFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
enum ReadEmailFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
enum ListLabelsFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

fn list_with_refresh(
    account: &Account,
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> std::result::Result<EmailListResponse, ListEmailsFailure> {
    let api = GmailApi::new().map_err(ListEmailsFailure::Gmail)?;
    let token = cached_or_refresh_token(account, state).map_err(ListEmailsFailure::Credential)?;
    match api.list_emails(&token, &account.email, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            invalidate_token(&account.subject, state);
            let token = refresh_token(account, state).map_err(ListEmailsFailure::Credential)?;
            api.list_emails(&token, &account.email, request)
                .map_err(ListEmailsFailure::Gmail)
        }
        Err(error) => Err(ListEmailsFailure::Gmail(error)),
    }
}

fn read_with_refresh(
    account: &Account,
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::EmailReadResponse, ReadEmailFailure> {
    let api = GmailApi::new().map_err(ReadEmailFailure::Gmail)?;
    let token = cached_or_refresh_token(account, state).map_err(ReadEmailFailure::Credential)?;
    match api.read_email(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            invalidate_token(&account.subject, state);
            let token = refresh_token(account, state).map_err(ReadEmailFailure::Credential)?;
            api.read_email(&token, request)
                .map_err(ReadEmailFailure::Gmail)
        }
        Err(error) => Err(ReadEmailFailure::Gmail(error)),
    }
}

fn list_labels_with_refresh(
    account: &Account,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::LabelListResponse, ListLabelsFailure> {
    let api = GmailApi::new().map_err(ListLabelsFailure::Gmail)?;
    let token = cached_or_refresh_token(account, state).map_err(ListLabelsFailure::Credential)?;
    match api.list_labels(&token) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            invalidate_token(&account.subject, state);
            let token = refresh_token(account, state).map_err(ListLabelsFailure::Credential)?;
            api.list_labels(&token).map_err(ListLabelsFailure::Gmail)
        }
        Err(error) => Err(ListLabelsFailure::Gmail(error)),
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
        "Gmail could not complete the request",
    )
}

fn map_read_email_error(error: &anyhow::Error) -> BrokerResponse {
    if is_read_email_too_large(error) {
        return BrokerResponse::error(
            BrokerErrorCode::MessageTooLarge,
            "the message exceeds the 256 KiB readable-body limit",
        );
    }
    if let Some(error) = error.downcast_ref::<GmailApiError>() {
        match error.status().as_u16() {
            400 => {
                return BrokerResponse::error(
                    BrokerErrorCode::InvalidRequest,
                    "Gmail rejected the message-read request",
                );
            }
            401 => {
                return BrokerResponse::error(
                    BrokerErrorCode::ReauthenticationRequired,
                    "reauthenticate the selected MCP target account in Arqen",
                );
            }
            404 => {
                return BrokerResponse::error(
                    BrokerErrorCode::MessageNotFound,
                    "Message not found in the currently selected account. Use an ID returned by list_emails.",
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
    }
    BrokerResponse::error(
        BrokerErrorCode::GmailUnavailable,
        "Gmail could not complete the message-read request",
    )
}

fn map_credential_error(error: &anyhow::Error) -> BrokerResponse {
    if is_invalid_grant(error) || is_missing_refresh_token(error) {
        return BrokerResponse::error(
            BrokerErrorCode::ReauthenticationRequired,
            "reauthenticate the selected MCP target account in Arqen",
        );
    }
    BrokerResponse::error(
        BrokerErrorCode::CredentialUnavailable,
        "the selected account credential is temporarily unavailable",
    )
}

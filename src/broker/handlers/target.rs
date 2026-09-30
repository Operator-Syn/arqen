// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(in crate::broker) fn has_gmail_modify_scope(account: &Account) -> bool {
    account.granted_scopes.as_deref().is_some_and(|scopes| {
        scopes
            .iter()
            .any(|scope| scope == crate::GMAIL_MODIFY_SCOPE)
    })
}

pub(in crate::broker) fn insufficient_scope_response() -> BrokerResponse {
    BrokerResponse::error(
        BrokerErrorCode::InsufficientScope,
        "reauthorize the selected Arqen account to grant Gmail modify access",
    )
}

pub(in crate::broker) fn account_database_unavailable() -> BrokerResponse {
    BrokerResponse::error(
        BrokerErrorCode::Internal,
        "the account database is unavailable",
    )
}

#[derive(Debug, Clone, Copy)]
pub(in crate::broker) enum TargetAccountFailure {
    NotConfigured,
    ConfigurationUnavailable,
    AccountUnavailable,
    AccountsUnavailable,
}

impl TargetAccountFailure {
    pub(in crate::broker) fn into_response(self) -> BrokerResponse {
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

pub(in crate::broker) fn selected_target_account(
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

pub(in crate::broker) fn target_ineligibility(account: &Account) -> Option<&'static str> {
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

// SPDX-License-Identifier: MPL-2.0
use super::super::*;

#[derive(Debug)]
pub(super) enum ListEmailsFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum ReadEmailFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum ListLabelsFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum MarkEmailFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum TrashEmailFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum DraftOperationFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

pub(super) fn draft_operation_with_refresh<T, F>(
    account: &Account,
    state: &BrokerState,
    operation: F,
) -> std::result::Result<T, DraftOperationFailure>
where
    F: Fn(&GmailApi, &str) -> Result<T>,
{
    let api = &state.api;
    let token =
        cached_or_refresh_token(account, state).map_err(DraftOperationFailure::Credential)?;
    match operation(api, &token) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(DraftOperationFailure::Credential)?;
            operation(api, &token).map_err(DraftOperationFailure::Gmail)
        }
        Err(error) => Err(DraftOperationFailure::Gmail(error)),
    }
}

#[derive(Debug)]
pub(super) enum LabelOperationFailure {
    Credential(anyhow::Error),
    Gmail(anyhow::Error),
}

#[derive(Debug)]
pub(super) enum ApplyLabelOperationFailure {
    Credential(anyhow::Error),
    Gmail(crate::gmail::ApplyLabelFailure),
}

pub(super) fn list_with_refresh(
    account: &Account,
    request: crate::gmail::ListEmailsRequest,
    state: &BrokerState,
) -> std::result::Result<EmailListResponse, ListEmailsFailure> {
    let api = &state.api;
    let token = cached_or_refresh_token(account, state).map_err(ListEmailsFailure::Credential)?;
    match api.list_emails(&token, &account.email, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(ListEmailsFailure::Credential)?;
            api.list_emails(&token, &account.email, request)
                .map_err(ListEmailsFailure::Gmail)
        }
        Err(error) => Err(ListEmailsFailure::Gmail(error)),
    }
}

pub(super) fn read_with_refresh(
    account: &Account,
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::EmailReadResponse, ReadEmailFailure> {
    let api = &state.api;
    let token = cached_or_refresh_token(account, state).map_err(ReadEmailFailure::Credential)?;
    match api.read_email(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(ReadEmailFailure::Credential)?;
            api.read_email(&token, request)
                .map_err(ReadEmailFailure::Gmail)
        }
        Err(error) => Err(ReadEmailFailure::Gmail(error)),
    }
}

pub(super) fn list_labels_with_refresh(
    account: &Account,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::LabelListResponse, ListLabelsFailure> {
    let api = &state.api;
    let token = cached_or_refresh_token(account, state).map_err(ListLabelsFailure::Credential)?;
    match api.list_labels(&token) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(ListLabelsFailure::Credential)?;
            api.list_labels(&token).map_err(ListLabelsFailure::Gmail)
        }
        Err(error) => Err(ListLabelsFailure::Gmail(error)),
    }
}

pub(super) fn create_label_with_refresh(
    account: &Account,
    request: crate::gmail::CreateLabelRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::EmailLabel, LabelOperationFailure> {
    let api = &state.api;
    let token =
        cached_or_refresh_token(account, state).map_err(LabelOperationFailure::Credential)?;
    match api.create_label(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(LabelOperationFailure::Credential)?;
            api.create_label(&token, request)
                .map_err(LabelOperationFailure::Gmail)
        }
        Err(error) => Err(LabelOperationFailure::Gmail(error)),
    }
}

pub(super) fn delete_label_with_refresh(
    account: &Account,
    request: crate::gmail::DeleteLabelRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::LabelDeleteResult, LabelOperationFailure> {
    let api = &state.api;
    let token =
        cached_or_refresh_token(account, state).map_err(LabelOperationFailure::Credential)?;
    match api.delete_label(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(LabelOperationFailure::Credential)?;
            api.delete_label(&token, request)
                .map_err(LabelOperationFailure::Gmail)
        }
        Err(error) => Err(LabelOperationFailure::Gmail(error)),
    }
}

pub(super) fn apply_label_with_refresh(
    account: &Account,
    request: crate::gmail::ApplyLabelRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::LabelApplyResult, ApplyLabelOperationFailure> {
    let api = &state.api;
    let token =
        cached_or_refresh_token(account, state).map_err(ApplyLabelOperationFailure::Credential)?;
    match api.apply_label(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if error.is_unauthorized() => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(ApplyLabelOperationFailure::Credential)?;
            api.apply_label(&token, request)
                .map_err(ApplyLabelOperationFailure::Gmail)
        }
        Err(error) => Err(ApplyLabelOperationFailure::Gmail(error)),
    }
}

pub(super) fn mark_email_with_refresh(
    account: &Account,
    request: crate::gmail::ReadEmailRequest,
    is_read: bool,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::EmailReadState, MarkEmailFailure> {
    let api = &state.api;
    let token = cached_or_refresh_token(account, state).map_err(MarkEmailFailure::Credential)?;
    let result = if is_read {
        api.mark_email_read(&token, request.clone())
    } else {
        api.mark_email_unread(&token, request.clone())
    };
    match result {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(MarkEmailFailure::Credential)?;
            let result = if is_read {
                api.mark_email_read(&token, request)
            } else {
                api.mark_email_unread(&token, request)
            };
            result.map_err(MarkEmailFailure::Gmail)
        }
        Err(error) => Err(MarkEmailFailure::Gmail(error)),
    }
}

pub(super) fn trash_email_with_refresh(
    account: &Account,
    request: crate::gmail::ReadEmailRequest,
    state: &BrokerState,
) -> std::result::Result<crate::gmail::EmailTrashResult, TrashEmailFailure> {
    let api = &state.api;
    let token = cached_or_refresh_token(account, state).map_err(TrashEmailFailure::Credential)?;
    match api.trash_email(&token, request.clone()) {
        Ok(result) => Ok(result),
        Err(error) if is_unauthorized(&error) => {
            let token = refresh_rejected_token(account, state, &token)
                .map_err(TrashEmailFailure::Credential)?;
            api.trash_email(&token, request)
                .map_err(TrashEmailFailure::Gmail)
        }
        Err(error) => Err(TrashEmailFailure::Gmail(error)),
    }
}

pub(super) fn cached_or_refresh_token(account: &Account, state: &BrokerState) -> Result<String> {
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
    refresh_token_using(account, state, None, || {
        refresh_google_access_token(
            &state.credentials_path,
            account.token_key.as_deref(),
            &account.subject,
        )
    })
}
fn refresh_token_using(
    account: &Account,
    state: &BrokerState,
    rejected: Option<&str>,
    refresh: impl FnOnce() -> Result<crate::auth::RefreshedAccessToken>,
) -> Result<String> {
    let lock = state
        .refresh_locks
        .lock()
        .map_err(|_| anyhow::anyhow!("refresh coordination is unavailable"))?
        .entry(account.subject.clone())
        .or_default()
        .clone();
    let _guard = lock
        .lock()
        .map_err(|_| anyhow::anyhow!("refresh coordination is unavailable"))?;
    if let Some(rejected) = rejected {
        let mut cache = state
            .access_tokens
            .lock()
            .map_err(|_| anyhow::anyhow!("token cache is unavailable"))?;
        if cache
            .get(&account.subject)
            .is_some_and(|token| token.value == rejected)
        {
            cache.remove(&account.subject);
        }
    }
    if let Some(token) = state
        .access_tokens
        .lock()
        .map_err(|_| anyhow::anyhow!("token cache is unavailable"))?
        .get(&account.subject)
        .filter(|t| t.expires_at > Instant::now())
        .map(|t| t.value.clone())
    {
        return Ok(token);
    }
    let refreshed = refresh()?;
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

fn refresh_rejected_token(
    account: &Account,
    state: &BrokerState,
    rejected: &str,
) -> Result<String> {
    refresh_token_using(account, state, Some(rejected), || {
        refresh_google_access_token(
            &state.credentials_path,
            account.token_key.as_deref(),
            &account.subject,
        )
    })
}
#[cfg(test)]
#[path = "../../../tests/unit/broker_bulk_refresh.rs"]
mod bulk_refresh_tests;

// SPDX-License-Identifier: MPL-2.0
use super::*;

#[cfg(unix)]
pub(super) fn handle_connection(stream: std::os::unix::net::UnixStream, state: &BrokerState) {
    let mut reader = BufReader::new(stream);
    let response = match read_request(&mut reader) {
        Ok(request) => match request.validate() {
            Ok(crate::mcp::BrokerRequest::ListEmails { request }) => {
                handle_list_emails(request, state)
            }
            Ok(crate::mcp::BrokerRequest::ListLabels) => handle_list_labels(state),
            Ok(crate::mcp::BrokerRequest::CreateLabel { request }) => {
                handle_create_label(request, state)
            }
            Ok(crate::mcp::BrokerRequest::DeleteLabel { request }) => {
                handle_delete_label(request, state)
            }
            Ok(crate::mcp::BrokerRequest::ApplyLabel { request }) => {
                handle_apply_label(request, state)
            }
            Ok(crate::mcp::BrokerRequest::ReadEmail { request }) => {
                handle_read_email(request, state)
            }
            Ok(crate::mcp::BrokerRequest::MarkEmailRead { request }) => {
                handle_mark_email_read(request, state)
            }
            Ok(crate::mcp::BrokerRequest::MarkEmailUnread { request }) => {
                handle_mark_email_unread(request, state)
            }
            Ok(crate::mcp::BrokerRequest::MarkEmailForDeletion { request }) => {
                handle_mark_email_for_deletion(request, state)
            }
            Ok(crate::mcp::BrokerRequest::DeleteMarkedEmail { request }) => {
                handle_delete_marked_email(request, state)
            }
            Ok(crate::mcp::BrokerRequest::ListDrafts { request }) => {
                handle_list_drafts(request, state)
            }
            Ok(crate::mcp::BrokerRequest::CreateDraft { request }) => {
                handle_create_draft(request, state)
            }
            Ok(crate::mcp::BrokerRequest::CreateReplyDraft { request }) => {
                handle_create_reply_draft(request, state)
            }
            Ok(crate::mcp::BrokerRequest::MarkDraftForDeletion { request }) => {
                handle_mark_draft_for_deletion(request, state)
            }
            Ok(crate::mcp::BrokerRequest::DeleteMarkedDraft { request }) => {
                handle_delete_marked_draft(request, state)
            }
            Ok(crate::mcp::BrokerRequest::MarkDraftForSending { request }) => {
                handle_mark_draft_for_sending(request, state)
            }
            Ok(crate::mcp::BrokerRequest::SendMarkedDraft { request }) => {
                handle_send_marked_draft(request, state)
            }
            Ok(crate::mcp::BrokerRequest::Readiness { .. }) => handle_readiness(state),
            Ok(request) => handle_bulk_request(request, state),
            Err(failure) => BrokerResponse::error(failure.code, failure.message),
        },
        Err(_) => BrokerResponse::error(
            BrokerErrorCode::InvalidRequest,
            "the broker request could not be read",
        ),
    };
    let response = if matches!(&response, BrokerResponse::EmailsRead { .. })
        && serde_json::to_vec(&response).map_or(true, |bytes| {
            bytes.len() + 1 > crate::gmail::MAX_BULK_RESULT_BYTES
        }) {
        BrokerResponse::error(
            BrokerErrorCode::Internal,
            "bulk read result exceeded its encoded budget",
        )
    } else {
        response
    };
    let mut stream = reader.into_inner();
    if let Ok(encoded) = serde_json::to_vec(&response)
        && encoded.len() < MAX_RESPONSE_BYTES
    {
        let _ = stream.write_all(&encoded);
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn handle_readiness(state: &BrokerState) -> BrokerResponse {
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
    if let Err(error) = check_google_refresh_token(account.token_key.as_deref(), &account.subject) {
        return map_credential_error(&error);
    }
    BrokerResponse::Ready
}

fn read_request<R: BufRead>(reader: &mut R) -> Result<BrokerRequest> {
    let line =
        read_bounded_line(reader, MAX_REQUEST_BYTES).context("read credential broker request")?;
    serde_json::from_slice(&line).context("parse credential broker request")
}

pub(super) fn read_bounded_line<R: BufRead>(reader: &mut R, limit: usize) -> Result<Vec<u8>> {
    let mut line = Vec::with_capacity(limit.min(4096));
    loop {
        let available = reader.fill_buf().context("read bounded line")?;
        if available.is_empty() {
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        anyhow::ensure!(
            line.len().saturating_add(take) <= limit,
            "credential broker request is too large"
        );
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    anyhow::ensure!(!line.is_empty(), "credential broker request was empty");
    Ok(line)
}

#[cfg(test)]
#[path = "../../tests/unit/broker_frames.rs"]
mod tests;

// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::gmail::concurrency::bounded_map;
impl GmailApi {
    pub(crate) fn apply_label_to_emails(
        &self,
        token: &str,
        request: ApplyLabelToEmailsRequest,
    ) -> Result<BulkResponse<LabelApplyResult>> {
        let request = request.validate()?;
        self.require_custom_label(token, &request.label_id)?;
        self.modify_message_set(token, &request.message_ids, &request.label_id, true, |id| {
            LabelApplyResult {
                message_id: id.into(),
                label_id: request.label_id.clone(),
                applied: true,
            }
        })
    }
    pub(crate) fn require_custom_label(&self, token: &str, id: &str) -> Result<()> {
        let mut url = self.base_url.join("users/me/labels")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("invalid provider path"))?
            .push(id);
        let label: LabelTypeResponse = self
            .client
            .get(url)
            .bearer_auth(token)
            .query(&[("fields", "id,type")])
            .send()
            .context("verify custom label")
            .and_then(parse_json_response)?;
        anyhow::ensure!(label.id == id, "provider identity mismatch");
        if label.label_type != EmailLabelType::User {
            return Err(SystemLabelError.into());
        }
        Ok(())
    }
    pub(crate) fn message_labels(&self, token: &str, id: &str) -> Result<Vec<String>> {
        let mut url = self.base_url.join("users/me/messages")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("invalid provider path"))?
            .push(id);
        let message: MessageLabelsResource = self
            .client
            .get(url)
            .bearer_auth(token)
            .query(&[("format", "minimal"), ("fields", "id,labelIds")])
            .send()
            .context("verify message state")
            .and_then(parse_json_response)?;
        anyhow::ensure!(message.id == id, "provider identity mismatch");
        Ok(message.label_ids)
    }
    pub(crate) fn mark_emails_state(
        &self,
        token: &str,
        request: BulkMessageIdsRequest,
        is_read: bool,
    ) -> Result<BulkResponse<EmailReadState>> {
        let request = request.validate()?;
        self.modify_message_set(token, &request.message_ids, "UNREAD", !is_read, |id| {
            EmailReadState {
                message_id: id.into(),
                is_read,
            }
        })
    }
    pub(crate) fn read_email_exact(&self, token: &str, id: &str) -> Result<EmailReadResponse> {
        let result = self.read_email(
            token,
            ReadEmailRequest {
                message_id: id.into(),
            },
        )?;
        anyhow::ensure!(result.message_id == id, "provider identity mismatch");
        Ok(result)
    }
    pub(crate) fn trash_email_verified(
        &self,
        token: &str,
        id: &str,
    ) -> Result<BulkOutcome<EmailTrashResult>> {
        let outcome = write_outcome(|| {
            self.trash_email(
                token,
                ReadEmailRequest {
                    message_id: id.into(),
                },
            )
        })?;
        match outcome {
            BulkOutcome::Succeeded { .. } | BulkOutcome::Unknown { .. } => {
                match self.message_labels(token, id) {
                    Ok(labels) if labels.iter().any(|l| l == "TRASH") => {
                        Ok(BulkOutcome::Succeeded {
                            result: EmailTrashResult {
                                message_id: id.into(),
                                trashed: true,
                            },
                        })
                    }
                    _ => Ok(unknown_write()),
                }
            }
            other => Ok(other),
        }
    }
    fn modify_message_set<T: Send>(
        &self,
        token: &str,
        ids: &[String],
        label: &str,
        add: bool,
        result: impl Fn(&str) -> T + Sync,
    ) -> Result<BulkResponse<T>> {
        let preflight = bounded_map(ids, |id| {
            self.message_labels(token, id).and_then(|labels| {
                if labels.iter().any(|l| l == "DRAFT") {
                    Err(DraftMessageMutationError.into())
                } else {
                    Ok(())
                }
            })
        });
        // This phase is read-only: propagate authentication rejection before
        // any batch is dispatched, so the broker can refresh safely once.
        if preflight
            .iter()
            .any(|check| check.as_ref().is_err_and(is_unauthorized))
        {
            return Err(preflight
                .into_iter()
                .find_map(|check| check.err().filter(is_unauthorized))
                .expect("authentication rejection was found"));
        }
        let eligible: Vec<_> = ids
            .iter()
            .zip(&preflight)
            .filter(|(_, r)| r.is_ok())
            .map(|(id, _)| id.clone())
            .collect();
        let write = if eligible.is_empty() {
            Ok(())
        } else {
            let body = if add {
                serde_json::json!({"ids":eligible,"addLabelIds":[label]})
            } else {
                serde_json::json!({"ids":eligible,"removeLabelIds":[label]})
            };
            self.client
                .post(self.base_url.join("users/me/messages/batchModify")?)
                .bearer_auth(token)
                .json(&body)
                .send()
                .context("native message mutation")
                .and_then(parse_empty_json_response)
        };
        // A 401 is a definitive rejection, safe to refresh before repeating this batch.
        if write.as_ref().is_err_and(is_unauthorized) {
            return Err(write.unwrap_err());
        }
        let definitive = write.as_ref().err().filter(|e| {
            e.downcast_ref::<GmailApiError>()
                .is_some_and(|e| (400..500).contains(&e.status().as_u16()))
        });
        let observed = if definitive.is_none() {
            bounded_map(&eligible, |id| self.message_labels(token, id))
        } else {
            vec![]
        };
        let mut observed = observed.into_iter();
        let items=ids.iter().zip(preflight).enumerate().map(|(index,(id,check))| {
   let outcome=match check {
    Err(error)=>read_failure(&error),
    Ok(())=>if let Some(error)=definitive{read_failure(error)}else{match observed.next().expect("one readback per eligible ID") {
     Ok(labels) if labels.iter().any(|l|l==label)==add => BulkOutcome::Succeeded { result:result(id) },
     _ => BulkOutcome::Unknown { code:"gmail_unavailable".into(),message:"the write was dispatched but desired state could not be verified; reconcile before retrying".into() },
    }},
   };BulkItem { index,outcome }
  }).collect();
        Ok(BulkResponse { items })
    }
}
pub(crate) fn read_failure<T>(error: &anyhow::Error) -> BulkOutcome<T> {
    let code = if is_read_email_too_large(error) {
        "message_too_large"
    } else if error.is::<DraftMessageMutationError>() {
        "invalid_request"
    } else if error.is::<SystemLabelError>() {
        "system_label"
    } else if let Some(e) = error.downcast_ref::<GmailApiError>() {
        match e.status().as_u16() {
            400 => "invalid_request",
            401 => "reauthentication_required",
            403 => "insufficient_scope",
            404 => "message_not_found",
            429 => "gmail_rate_limited",
            _ => "gmail_unavailable",
        }
    } else {
        "gmail_unavailable"
    };
    BulkOutcome::Failed {
        code: code.into(),
        message: "the provider operation could not be completed".into(),
    }
}
pub(crate) fn write_outcome<T>(operation: impl FnOnce() -> Result<T>) -> Result<BulkOutcome<T>> {
    write_outcome_mapped(operation, read_failure)
}
pub(crate) fn write_outcome_as<T>(
    operation: impl FnOnce() -> Result<T>,
    failure: impl FnOnce(&anyhow::Error) -> BulkOutcome<T>,
) -> Result<BulkOutcome<T>> {
    write_outcome_mapped(operation, failure)
}
pub(crate) fn write_outcome_mapped<T>(
    operation: impl FnOnce() -> Result<T>,
    failure: impl FnOnce(&anyhow::Error) -> BulkOutcome<T>,
) -> Result<BulkOutcome<T>> {
    let before = transport::dispatched_writes();
    match operation() {
        Ok(result) => Ok(BulkOutcome::Succeeded { result }),
        Err(error) if is_unauthorized(&error) => Err(error),
        Err(error)
            if transport::dispatched_writes() == before
                || error
                    .downcast_ref::<GmailApiError>()
                    .is_some_and(|e| (400..500).contains(&e.status().as_u16())) =>
        {
            Ok(failure(&error))
        }
        Err(_) => Ok(unknown_write()),
    }
}
pub(crate) fn unknown_write<T>() -> BulkOutcome<T> {
    BulkOutcome::Unknown {
        code: "gmail_unavailable".into(),
        message: "the write was dispatched but its effect is unverified; reconcile before retrying"
            .into(),
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/gmail_bulk_provider.rs"]
mod tests;

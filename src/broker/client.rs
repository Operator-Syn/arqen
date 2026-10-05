// SPDX-License-Identifier: MPL-2.0
use super::*;
mod bulk;

#[cfg(unix)]
#[derive(Clone)]
pub struct BrokerClient {
    socket_path: PathBuf,
}

#[cfg(unix)]
impl BrokerClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub async fn list_emails(
        &self,
        request: crate::gmail::ListEmailsRequest,
    ) -> std::result::Result<EmailListResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request =
            BrokerRequest::list_emails(request.validate().map_err(|_| BrokerFailure {
                code: BrokerErrorCode::InvalidRequest,
                message: "the mail-list request is invalid".into(),
            })?);
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response was invalid".into(),
            })?;
        match response {
            BrokerResponse::Ok { result } => Ok(result),
            BrokerResponse::ReadEmail { .. }
            | BrokerResponse::MessageReadState { .. }
            | BrokerResponse::Labels { .. }
            | BrokerResponse::LabelCreated { .. }
            | BrokerResponse::LabelDeleted { .. }
            | BrokerResponse::LabelApplied { .. }
            | BrokerResponse::DeletionMarked { .. }
            | BrokerResponse::EmailTrashed { .. }
            | BrokerResponse::Drafts { .. }
            | BrokerResponse::DraftCreated { .. }
            | BrokerResponse::DraftDeletionMarked { .. }
            | BrokerResponse::DraftSendingMarked { .. }
            | BrokerResponse::DraftDeleted { .. }
            | BrokerResponse::DraftSent { .. }
            | BrokerResponse::EmailsRead { .. }
            | BrokerResponse::LabelsApplied { .. }
            | BrokerResponse::MessagesReadState { .. }
            | BrokerResponse::EmailsDeletionMarked { .. }
            | BrokerResponse::EmailsTrashed { .. }
            | BrokerResponse::LabelsCreated { .. }
            | BrokerResponse::LabelsDeleted { .. }
            | BrokerResponse::DraftsCreated { .. }
            | BrokerResponse::DraftsDeletionMarked { .. }
            | BrokerResponse::DraftsSendingMarked { .. }
            | BrokerResponse::DraftsDeleted { .. }
            | BrokerResponse::DraftsSent { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid mail-list response".into(),
            }),
            BrokerResponse::Ready => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid mail-list response".into(),
            }),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
        }
    }

    pub async fn list_labels(
        &self,
    ) -> std::result::Result<crate::gmail::LabelListResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request = BrokerRequest::list_labels();
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response was invalid".into(),
            })?;
        match response {
            BrokerResponse::Labels { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            BrokerResponse::Ready
            | BrokerResponse::Ok { .. }
            | BrokerResponse::ReadEmail { .. }
            | BrokerResponse::MessageReadState { .. }
            | BrokerResponse::LabelCreated { .. }
            | BrokerResponse::LabelDeleted { .. }
            | BrokerResponse::LabelApplied { .. }
            | BrokerResponse::DeletionMarked { .. }
            | BrokerResponse::EmailTrashed { .. }
            | BrokerResponse::Drafts { .. }
            | BrokerResponse::DraftCreated { .. }
            | BrokerResponse::DraftDeletionMarked { .. }
            | BrokerResponse::DraftSendingMarked { .. }
            | BrokerResponse::DraftDeleted { .. }
            | BrokerResponse::DraftSent { .. }
            | BrokerResponse::EmailsRead { .. }
            | BrokerResponse::LabelsApplied { .. }
            | BrokerResponse::MessagesReadState { .. }
            | BrokerResponse::EmailsDeletionMarked { .. }
            | BrokerResponse::EmailsTrashed { .. }
            | BrokerResponse::LabelsCreated { .. }
            | BrokerResponse::LabelsDeleted { .. }
            | BrokerResponse::DraftsCreated { .. }
            | BrokerResponse::DraftsDeletionMarked { .. }
            | BrokerResponse::DraftsSendingMarked { .. }
            | BrokerResponse::DraftsDeleted { .. }
            | BrokerResponse::DraftsSent { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid label-list response".into(),
            }),
        }
    }

    pub async fn create_label(
        &self,
        request: crate::gmail::CreateLabelRequest,
    ) -> std::result::Result<crate::gmail::EmailLabel, BrokerFailure> {
        let request =
            BrokerRequest::create_label(request.validate().map_err(|_| BrokerFailure {
                code: BrokerErrorCode::InvalidLabelName,
                message: "name must be nonblank and contain no control characters".into(),
            })?);
        match self.exchange_label_request(request).await? {
            BrokerResponse::LabelCreated { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid label-create response".into(),
            }),
        }
    }

    pub async fn delete_label(
        &self,
        request: crate::gmail::DeleteLabelRequest,
    ) -> std::result::Result<crate::gmail::LabelDeleteResult, BrokerFailure> {
        let request =
            BrokerRequest::delete_label(request.validate().map_err(|_| BrokerFailure {
                code: BrokerErrorCode::InvalidLabelId,
                message: "label_id must be nonempty and contain no control characters".into(),
            })?);
        match self.exchange_label_request(request).await? {
            BrokerResponse::LabelDeleted { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid label-delete response".into(),
            }),
        }
    }

    pub async fn apply_label(
        &self,
        request: crate::gmail::ApplyLabelRequest,
    ) -> std::result::Result<crate::gmail::LabelApplyResult, BrokerFailure> {
        request.validate_message_id().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidMessageId,
            message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
                .into(),
        })?;
        request.validate_label_id().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidLabelId,
            message: "label_id must be nonempty and contain no control characters".into(),
        })?;
        match self
            .exchange_label_request(BrokerRequest::apply_label(request))
            .await?
        {
            BrokerResponse::LabelApplied { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid label-apply response".into(),
            }),
        }
    }

    pub async fn mark_email_for_deletion(
        &self,
        request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailDeletionMark, BrokerFailure> {
        let request = request.validate().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidMessageId,
            message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
                .into(),
        })?;
        match self
            .exchange_label_request(BrokerRequest::mark_email_for_deletion(request))
            .await?
        {
            BrokerResponse::DeletionMarked { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid email-deletion mark response".into(),
            }),
        }
    }

    pub async fn delete_marked_email(
        &self,
        request: crate::gmail::DeleteMarkedEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailTrashResult, BrokerFailure> {
        let request = request.validate().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidDeletionMark,
            message: "marker_id must be a 32-character deletion marker returned by mark_email_for_deletion".into(),
        })?;
        match self
            .exchange_label_request(BrokerRequest::delete_marked_email(request))
            .await?
        {
            BrokerResponse::EmailTrashed { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid email-trash response".into(),
            }),
        }
    }

    pub async fn list_drafts(
        &self,
        request: crate::gmail::ListDraftsRequest,
    ) -> std::result::Result<crate::gmail::DraftListResponse, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::list_drafts(request.validate().map_err(
                |_| BrokerFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the draft-list request is invalid".into(),
                },
            )?))
            .await?
        {
            BrokerResponse::Drafts { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-list")),
        }
    }

    pub async fn create_draft(
        &self,
        request: crate::gmail::CreateDraftRequest,
    ) -> std::result::Result<crate::gmail::DraftCreateResult, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::create_draft(request.validate().map_err(
                |_| BrokerFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the draft content is invalid".into(),
                },
            )?))
            .await?
        {
            BrokerResponse::DraftCreated { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-create")),
        }
    }

    pub async fn create_reply_draft(
        &self,
        request: crate::gmail::CreateReplyDraftRequest,
    ) -> std::result::Result<crate::gmail::DraftCreateResult, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::create_reply_draft(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the reply-draft request is invalid".into(),
                })?,
            ))
            .await?
        {
            BrokerResponse::DraftCreated { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("reply-draft-create")),
        }
    }

    pub async fn mark_draft_for_deletion(
        &self,
        request: crate::gmail::DraftIdRequest,
    ) -> std::result::Result<crate::gmail::DraftActionMark, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::mark_draft_for_deletion(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the draft ID is invalid".into(),
                })?,
            ))
            .await?
        {
            BrokerResponse::DraftDeletionMarked { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-deletion-mark")),
        }
    }

    pub async fn delete_marked_draft(
        &self,
        request: crate::gmail::ActionMarkerRequest,
    ) -> std::result::Result<crate::gmail::DraftDeleteResult, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::delete_marked_draft(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidActionMarker,
                    message: "the action marker is invalid".into(),
                })?,
            ))
            .await?
        {
            BrokerResponse::DraftDeleted { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-delete")),
        }
    }

    pub async fn mark_draft_for_sending(
        &self,
        request: crate::gmail::DraftIdRequest,
    ) -> std::result::Result<crate::gmail::DraftActionMark, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::mark_draft_for_sending(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidRequest,
                    message: "the draft ID is invalid".into(),
                })?,
            ))
            .await?
        {
            BrokerResponse::DraftSendingMarked { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-send-mark")),
        }
    }

    pub async fn send_marked_draft(
        &self,
        request: crate::gmail::ActionMarkerRequest,
    ) -> std::result::Result<crate::gmail::DraftSendResult, BrokerFailure> {
        match self
            .exchange_label_request(BrokerRequest::send_marked_draft(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidActionMarker,
                    message: "the action marker is invalid".into(),
                })?,
            ))
            .await?
        {
            BrokerResponse::DraftSent { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            _ => Err(invalid_broker_response("draft-send")),
        }
    }

    async fn exchange_label_request(
        &self,
        request: BrokerRequest,
    ) -> std::result::Result<BrokerResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker response could not be read".into(),
            })?;
        serde_json::from_slice(&line).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker response was invalid".into(),
        })
    }

    pub async fn read_email(
        &self,
        request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request =
            BrokerRequest::read_email(
                request.validate().map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::InvalidMessageId,
                    message:
                        "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
                            .into(),
                })?,
            );
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker response was invalid".into(),
            })?;
        match response {
            BrokerResponse::ReadEmail { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            BrokerResponse::Ready
            | BrokerResponse::Ok { .. }
            | BrokerResponse::Labels { .. }
            | BrokerResponse::MessageReadState { .. }
            | BrokerResponse::LabelCreated { .. }
            | BrokerResponse::LabelDeleted { .. }
            | BrokerResponse::LabelApplied { .. }
            | BrokerResponse::DeletionMarked { .. }
            | BrokerResponse::EmailTrashed { .. }
            | BrokerResponse::Drafts { .. }
            | BrokerResponse::DraftCreated { .. }
            | BrokerResponse::DraftDeletionMarked { .. }
            | BrokerResponse::DraftSendingMarked { .. }
            | BrokerResponse::DraftDeleted { .. }
            | BrokerResponse::DraftSent { .. }
            | BrokerResponse::EmailsRead { .. }
            | BrokerResponse::LabelsApplied { .. }
            | BrokerResponse::MessagesReadState { .. }
            | BrokerResponse::EmailsDeletionMarked { .. }
            | BrokerResponse::EmailsTrashed { .. }
            | BrokerResponse::LabelsCreated { .. }
            | BrokerResponse::LabelsDeleted { .. }
            | BrokerResponse::DraftsCreated { .. }
            | BrokerResponse::DraftsDeletionMarked { .. }
            | BrokerResponse::DraftsSendingMarked { .. }
            | BrokerResponse::DraftsDeleted { .. }
            | BrokerResponse::DraftsSent { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid message-read response".into(),
            }),
        }
    }

    pub async fn mark_email_read(
        &self,
        request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadState, BrokerFailure> {
        self.mark_email_state(request, true).await
    }

    pub async fn mark_email_unread(
        &self,
        request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadState, BrokerFailure> {
        self.mark_email_state(request, false).await
    }

    async fn mark_email_state(
        &self,
        request: crate::gmail::ReadEmailRequest,
        is_read: bool,
    ) -> std::result::Result<crate::gmail::EmailReadState, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request = request.validate().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidMessageId,
            message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
                .into(),
        })?;
        let request = if is_read {
            BrokerRequest::mark_email_read(request)
        } else {
            BrokerRequest::mark_email_unread(request)
        };
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker response was invalid".into(),
            })?;
        match response {
            BrokerResponse::MessageReadState { result } => Ok(result),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            BrokerResponse::Ready
            | BrokerResponse::Ok { .. }
            | BrokerResponse::Labels { .. }
            | BrokerResponse::ReadEmail { .. }
            | BrokerResponse::LabelCreated { .. }
            | BrokerResponse::LabelDeleted { .. }
            | BrokerResponse::LabelApplied { .. }
            | BrokerResponse::DeletionMarked { .. }
            | BrokerResponse::EmailTrashed { .. }
            | BrokerResponse::Drafts { .. }
            | BrokerResponse::DraftCreated { .. }
            | BrokerResponse::DraftDeletionMarked { .. }
            | BrokerResponse::DraftSendingMarked { .. }
            | BrokerResponse::DraftDeleted { .. }
            | BrokerResponse::DraftSent { .. }
            | BrokerResponse::EmailsRead { .. }
            | BrokerResponse::LabelsApplied { .. }
            | BrokerResponse::MessagesReadState { .. }
            | BrokerResponse::EmailsDeletionMarked { .. }
            | BrokerResponse::EmailsTrashed { .. }
            | BrokerResponse::LabelsCreated { .. }
            | BrokerResponse::LabelsDeleted { .. }
            | BrokerResponse::DraftsCreated { .. }
            | BrokerResponse::DraftsDeletionMarked { .. }
            | BrokerResponse::DraftsSendingMarked { .. }
            | BrokerResponse::DraftsDeleted { .. }
            | BrokerResponse::DraftsSent { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid message-state response".into(),
            }),
        }
    }

    pub async fn readiness(&self) -> std::result::Result<(), BrokerFailure> {
        match tokio::time::timeout(Duration::from_secs(2), self.readiness_inner()).await {
            Ok(result) => result,
            Err(_) => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the credential broker readiness check timed out".into(),
            }),
        }
    }

    async fn readiness_inner(&self) -> std::result::Result<(), BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request = BrokerRequest::readiness();
        let mut stream =
            UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| BrokerFailure {
                    code: BrokerErrorCode::Internal,
                    message: "the credential broker is unavailable".into(),
                })?;
        let mut encoded = serde_json::to_vec(&request).map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker readiness request could not be encoded".into(),
        })?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness request failed".into(),
            })?;
        stream.shutdown().await.map_err(|_| BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the broker readiness request could not finish".into(),
        })?;
        let mut reader = BufReader::new(stream);
        let line = read_bounded_line_async(&mut reader, MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness response could not be read".into(),
            })?;
        let response: BrokerResponse =
            serde_json::from_slice(&line).map_err(|_| BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker readiness response was invalid".into(),
            })?;
        match response {
            BrokerResponse::Ready => Ok(()),
            BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
            BrokerResponse::Ok { .. }
            | BrokerResponse::ReadEmail { .. }
            | BrokerResponse::Labels { .. }
            | BrokerResponse::MessageReadState { .. }
            | BrokerResponse::LabelCreated { .. }
            | BrokerResponse::LabelDeleted { .. }
            | BrokerResponse::LabelApplied { .. }
            | BrokerResponse::DeletionMarked { .. }
            | BrokerResponse::EmailTrashed { .. }
            | BrokerResponse::Drafts { .. }
            | BrokerResponse::DraftCreated { .. }
            | BrokerResponse::DraftDeletionMarked { .. }
            | BrokerResponse::DraftSendingMarked { .. }
            | BrokerResponse::DraftDeleted { .. }
            | BrokerResponse::DraftSent { .. }
            | BrokerResponse::EmailsRead { .. }
            | BrokerResponse::LabelsApplied { .. }
            | BrokerResponse::MessagesReadState { .. }
            | BrokerResponse::EmailsDeletionMarked { .. }
            | BrokerResponse::EmailsTrashed { .. }
            | BrokerResponse::LabelsCreated { .. }
            | BrokerResponse::LabelsDeleted { .. }
            | BrokerResponse::DraftsCreated { .. }
            | BrokerResponse::DraftsDeletionMarked { .. }
            | BrokerResponse::DraftsSendingMarked { .. }
            | BrokerResponse::DraftsDeleted { .. }
            | BrokerResponse::DraftsSent { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid readiness response".into(),
            }),
        }
    }
}

#[cfg(not(unix))]
fn unsupported_host() -> BrokerFailure {
    BrokerFailure {
        code: BrokerErrorCode::Internal,
        message: "the credential broker currently requires a Unix host".into(),
    }
}

#[cfg(unix)]
fn invalid_broker_response(operation: &str) -> BrokerFailure {
    BrokerFailure {
        code: BrokerErrorCode::Internal,
        message: format!("the broker returned an invalid {operation} response"),
    }
}

#[cfg(not(unix))]
#[derive(Clone)]
pub struct BrokerClient {
    _socket_path: PathBuf,
}

#[cfg(not(unix))]
impl BrokerClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            _socket_path: socket_path.into(),
        }
    }

    pub async fn list_emails(
        &self,
        _request: crate::gmail::ListEmailsRequest,
    ) -> std::result::Result<EmailListResponse, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn mark_email_for_deletion(
        &self,
        _request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailDeletionMark, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn delete_marked_email(
        &self,
        _request: crate::gmail::DeleteMarkedEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailTrashResult, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn list_labels(
        &self,
    ) -> std::result::Result<crate::gmail::LabelListResponse, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn create_label(
        &self,
        _request: crate::gmail::CreateLabelRequest,
    ) -> std::result::Result<crate::gmail::EmailLabel, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn delete_label(
        &self,
        _request: crate::gmail::DeleteLabelRequest,
    ) -> std::result::Result<crate::gmail::LabelDeleteResult, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn apply_label(
        &self,
        _request: crate::gmail::ApplyLabelRequest,
    ) -> std::result::Result<crate::gmail::LabelApplyResult, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn read_email(
        &self,
        _request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadResponse, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn mark_email_read(
        &self,
        _request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadState, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn mark_email_unread(
        &self,
        _request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadState, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }

    pub async fn list_drafts(
        &self,
        _request: crate::gmail::ListDraftsRequest,
    ) -> std::result::Result<crate::gmail::DraftListResponse, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn create_draft(
        &self,
        _request: crate::gmail::CreateDraftRequest,
    ) -> std::result::Result<crate::gmail::DraftCreateResult, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn create_reply_draft(
        &self,
        _request: crate::gmail::CreateReplyDraftRequest,
    ) -> std::result::Result<crate::gmail::DraftCreateResult, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn mark_draft_for_deletion(
        &self,
        _request: crate::gmail::DraftIdRequest,
    ) -> std::result::Result<crate::gmail::DraftActionMark, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn delete_marked_draft(
        &self,
        _request: crate::gmail::ActionMarkerRequest,
    ) -> std::result::Result<crate::gmail::DraftDeleteResult, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn mark_draft_for_sending(
        &self,
        _request: crate::gmail::DraftIdRequest,
    ) -> std::result::Result<crate::gmail::DraftActionMark, BrokerFailure> {
        Err(unsupported_host())
    }
    pub async fn send_marked_draft(
        &self,
        _request: crate::gmail::ActionMarkerRequest,
    ) -> std::result::Result<crate::gmail::DraftSendResult, BrokerFailure> {
        Err(unsupported_host())
    }

    pub async fn readiness(&self) -> std::result::Result<(), BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
    }
}

#[cfg(unix)]
async fn read_bounded_line_async<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncBufReadExt;

    let mut line = Vec::with_capacity(limit.min(4096));
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        if line.len().saturating_add(take) > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "credential broker response is too large",
            ));
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    if line.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "credential broker response was empty",
        ));
    }
    Ok(line)
}

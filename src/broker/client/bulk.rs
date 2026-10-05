// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::gmail::*;
impl BrokerClient {
    pub async fn read_emails(
        &self,
        request: ReadEmailsRequest,
    ) -> Result<BulkResponse<EmailReadResponse>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::ReadEmails { request })
                .await?
            {
                BrokerResponse::EmailsRead { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("read_emails")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn apply_label_to_emails(
        &self,
        request: ApplyLabelToEmailsRequest,
    ) -> Result<BulkResponse<LabelApplyResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::ApplyLabelToEmails { request })
                .await?
            {
                BrokerResponse::LabelsApplied { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("apply_label_to_emails")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn mark_emails_read(
        &self,
        request: BulkMessageIdsRequest,
    ) -> Result<BulkResponse<EmailReadState>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::MarkEmailsRead { request })
                .await?
            {
                BrokerResponse::MessagesReadState { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("mark_emails_read")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn mark_emails_unread(
        &self,
        request: BulkMessageIdsRequest,
    ) -> Result<BulkResponse<EmailReadState>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::MarkEmailsUnread { request })
                .await?
            {
                BrokerResponse::MessagesReadState { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("mark_emails_unread")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn mark_emails_for_deletion(
        &self,
        request: BulkMessageIdsRequest,
    ) -> Result<BulkResponse<EmailDeletionMark>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::MarkEmailsForDeletion { request })
                .await?
            {
                BrokerResponse::EmailsDeletionMarked { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("mark_emails_for_deletion")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn delete_marked_emails(
        &self,
        request: BulkEmailMarkersRequest,
    ) -> Result<BulkResponse<EmailTrashResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::DeleteMarkedEmails { request })
                .await?
            {
                BrokerResponse::EmailsTrashed { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("delete_marked_emails")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn create_labels(
        &self,
        request: CreateLabelsRequest,
    ) -> Result<BulkResponse<EmailLabel>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::CreateLabels { request })
                .await?
            {
                BrokerResponse::LabelsCreated { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("create_labels")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn delete_labels(
        &self,
        request: DeleteLabelsRequest,
    ) -> Result<BulkResponse<LabelDeleteResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::DeleteLabels { request })
                .await?
            {
                BrokerResponse::LabelsDeleted { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("delete_labels")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn create_drafts(
        &self,
        request: CreateDraftsRequest,
    ) -> Result<BulkResponse<DraftCreateResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::CreateDrafts { request })
                .await?
            {
                BrokerResponse::DraftsCreated { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("create_drafts")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn create_reply_drafts(
        &self,
        request: CreateReplyDraftsRequest,
    ) -> Result<BulkResponse<DraftCreateResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::CreateReplyDrafts { request })
                .await?
            {
                BrokerResponse::DraftsCreated { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("create_reply_drafts")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn mark_drafts_for_deletion(
        &self,
        request: BulkDraftIdsRequest,
    ) -> Result<BulkResponse<DraftActionMark>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::MarkDraftsForDeletion { request })
                .await?
            {
                BrokerResponse::DraftsDeletionMarked { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("mark_drafts_for_deletion")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn mark_drafts_for_sending(
        &self,
        request: BulkDraftIdsRequest,
    ) -> Result<BulkResponse<DraftActionMark>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::MarkDraftsForSending { request })
                .await?
            {
                BrokerResponse::DraftsSendingMarked { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("mark_drafts_for_sending")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn delete_marked_drafts(
        &self,
        request: BulkDraftMarkersRequest,
    ) -> Result<BulkResponse<DraftDeleteResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::DeleteMarkedDrafts { request })
                .await?
            {
                BrokerResponse::DraftsDeleted { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("delete_marked_drafts")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    pub async fn send_marked_drafts(
        &self,
        request: BulkDraftMarkersRequest,
    ) -> Result<BulkResponse<DraftSendResult>, BrokerFailure> {
        #[cfg(unix)]
        {
            match self
                .exchange_bulk(BrokerRequest::SendMarkedDrafts { request })
                .await?
            {
                BrokerResponse::DraftsSent { result } => Ok(result),
                BrokerResponse::Error { code, message } => Err(BrokerFailure { code, message }),
                _ => Err(invalid_broker_response("send_marked_drafts")),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = request;
            Err(unsupported_host())
        }
    }
    #[cfg(unix)]
    async fn exchange_bulk(&self, request: BrokerRequest) -> Result<BrokerResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        let request = request.validate().map_err(|e| BrokerFailure {
            code: e.code,
            message: e.message.into(),
        })?;
        let mut encoded = serde_json::to_vec(&request)
            .map_err(|_| invalid_broker_response("request encoding"))?;
        encoded.push(b'\n');
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err(BrokerFailure {
                code: BrokerErrorCode::InvalidRequest,
                message: "the encoded bulk request exceeds the broker frame limit".into(),
            });
        }
        let mut stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(|_| invalid_broker_response("connection"))?;
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| invalid_broker_response("write"))?;
        stream
            .shutdown()
            .await
            .map_err(|_| invalid_broker_response("write completion"))?;
        let frame = read_bounded_line_async(&mut BufReader::new(stream), MAX_RESPONSE_BYTES)
            .await
            .map_err(|_| invalid_broker_response("read"))?;
        serde_json::from_slice(&frame).map_err(|_| invalid_broker_response("decoding"))
    }
}

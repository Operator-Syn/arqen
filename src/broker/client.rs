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
            BrokerResponse::ReadEmail { .. } => Err(BrokerFailure {
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

    pub async fn read_email(
        &self,
        request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadResponse, BrokerFailure> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let request = BrokerRequest::read_email(request.validate().map_err(|_| BrokerFailure {
            code: BrokerErrorCode::InvalidMessageId,
            message: "message_id must be 1–256 ASCII letters, digits, hyphens, or underscores"
                .into(),
        })?);
        let mut stream = UnixStream::connect(&self.socket_path)
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
            BrokerResponse::Ready | BrokerResponse::Ok { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid message-read response".into(),
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
            BrokerResponse::Ok { .. } | BrokerResponse::ReadEmail { .. } => Err(BrokerFailure {
                code: BrokerErrorCode::Internal,
                message: "the broker returned an invalid readiness response".into(),
            }),
        }
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

    pub async fn read_email(
        &self,
        _request: crate::gmail::ReadEmailRequest,
    ) -> std::result::Result<crate::gmail::EmailReadResponse, BrokerFailure> {
        Err(BrokerFailure {
            code: BrokerErrorCode::Internal,
            message: "the credential broker currently requires a Unix host".into(),
        })
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

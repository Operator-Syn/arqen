async fn websocket_proxy<S>(client: WebSocket, upstream: WebSocketStream<S>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut upstream_sink, mut upstream_stream) = upstream.split();
    let (mut client_sink, mut client_stream) = client.split();
    let client_to_upstream = async {
        while let Some(Ok(message)) = client_stream.next().await {
            if upstream_sink
                .send(to_upstream_message(message))
                .await
                .is_err()
            {
                break;
            }
        }
    };
    let upstream_to_client = async {
        while let Some(Ok(message)) = upstream_stream.next().await {
            let Some(message) = from_upstream_message(message) else {
                continue;
            };
            if client_sink.send(message).await.is_err() {
                break;
            }
        }
    };
    tokio::pin!(client_to_upstream);
    tokio::pin!(upstream_to_client);
    tokio::select! {
        _ = &mut client_to_upstream => {},
        _ = &mut upstream_to_client => {},
    }
}

fn to_upstream_message(message: AxumMessage) -> UpstreamMessage {
    match message {
        AxumMessage::Text(text) => UpstreamMessage::Text(text.to_string().into()),
        AxumMessage::Binary(data) => UpstreamMessage::Binary(data),
        AxumMessage::Ping(data) => UpstreamMessage::Ping(data),
        AxumMessage::Pong(data) => UpstreamMessage::Pong(data),
        AxumMessage::Close(None) => UpstreamMessage::Close(None),
        AxumMessage::Close(Some(frame)) => UpstreamMessage::Close(Some(UpstreamCloseFrame {
            code: frame.code.into(),
            reason: frame.reason.to_string().into(),
        })),
    }
}

fn from_upstream_message(message: UpstreamMessage) -> Option<AxumMessage> {
    match message {
        UpstreamMessage::Text(text) => Some(AxumMessage::text(text.to_string())),
        UpstreamMessage::Binary(data) => Some(AxumMessage::binary(data)),
        UpstreamMessage::Ping(data) => Some(AxumMessage::Ping(data)),
        UpstreamMessage::Pong(data) => Some(AxumMessage::Pong(data)),
        UpstreamMessage::Close(_) => Some(AxumMessage::Close(None)),
        UpstreamMessage::Frame(_) => None,
    }
}

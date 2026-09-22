pub fn run(options: ServerOptions) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create MCP server runtime")?;
    runtime.block_on(run_async(options))
}

async fn run_async(options: ServerOptions) -> anyhow::Result<()> {
    let cancellation_token = CancellationToken::new();
    let listen_addr = options.listen_addr;
    let router = build_router(options, cancellation_token.clone());
    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("bind MCP server at {listen_addr}"))?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(cancellation_token))
        .await
        .context("serve Streamable HTTP MCP endpoint")
}

fn build_router(options: ServerOptions, cancellation_token: CancellationToken) -> Router {
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(MAX_MCP_REQUEST_BYTES)
        .with_allowed_hosts(options.allowed_hosts)
        .with_allowed_origins(options.allowed_origins)
        .with_cancellation_token(cancellation_token);
    let broker = BrokerClient::new(options.broker_socket);
    let service_broker = broker.clone();
    let service = StreamableHttpService::new(
        move || Ok(EmailMcpServer::new(service_broker.clone())),
        LocalSessionManager::default().into(),
        config,
    );
    let auth_state = AuthState {
        bearer_token: Arc::new(options.bearer_token),
    };
    Router::new()
        .nest_service("/mcp", service)
        .route("/healthz", get(health))
        .route(
            "/readyz",
            get(move || {
                let broker = broker.clone();
                async move { readiness(broker).await }
            }),
        )
        .layer(from_fn_with_state(auth_state, authorize))
}

async fn health() -> impl IntoResponse {
    (StatusCode::NO_CONTENT, ())
}

async fn readiness(broker: BrokerClient) -> Response {
    match broker.readiness().await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "code": error.code.as_str(),
                "message": error.message,
            })),
        )
            .into_response(),
    }
}

async fn authorize(State(state): State<AuthState>, request: Request, next: Next) -> Response {
    if request_is_authorized(&request, &state.bearer_token) {
        return next.run(request).await;
    }
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response.headers_mut().insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"arqen-mcp\""),
    );
    response
}

fn request_is_authorized<B>(request: &axum::http::Request<B>, expected: &str) -> bool {
    let Some(value) = request.headers().get("authorization") else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(provided) = value.strip_prefix("Bearer ") else {
        return false;
    };
    !provided.is_empty() && provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

async fn shutdown_signal(cancellation_token: CancellationToken) {
    let _ = tokio::signal::ctrl_c().await;
    cancellation_token.cancel();
}

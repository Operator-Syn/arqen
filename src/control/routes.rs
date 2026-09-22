fn build_router(state: GatewayState) -> Router {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/ws", get(websocket))
        .fallback(any(proxy_http))
        .with_state(state)
}

async fn login(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    {
        let mut limiter = state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(retry_after) = limiter.retry_after() {
            let mut response = login_response(
                StatusCode::TOO_MANY_REQUESTS,
                Some("Too many attempts. Wait a moment and try again."),
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after.as_secs().max(1).to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            return response;
        }
    }
    let body = match to_bytes(request.into_body(), MAX_LOGIN_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            state
                .rate_limiter
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .record_failure();
            return login_response(
                StatusCode::BAD_REQUEST,
                Some("Enter a username and password."),
            );
        }
    };
    let Some((username, password)) = parse_login_form(&body) else {
        state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record_failure();
        return login_response(
            StatusCode::UNAUTHORIZED,
            Some("Credentials not recognized."),
        );
    };
    let valid_username = username.as_bytes().ct_eq(b"arqen");
    let valid_password = password.as_bytes().ct_eq(state.password.as_bytes());
    if !bool::from(valid_username & valid_password) {
        state
            .rate_limiter
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .record_failure();
        return login_response(
            StatusCode::UNAUTHORIZED,
            Some("Credentials not recognized."),
        );
    }
    state
        .rate_limiter
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .reset();
    let token = state.sessions.issue();
    redirect_with_cookie(StatusCode::SEE_OTHER, session_cookie(&token))
}

async fn logout(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if let Some(token) = session_token(request.headers()) {
        state.sessions.revoke(&token);
    }
    redirect_with_cookie(StatusCode::SEE_OTHER, expired_session_cookie())
}

async fn websocket(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    uri: Uri,
    mut upgrade: WebSocketUpgrade,
) -> Response {
    if !origin_is_allowed(&headers) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if !is_authenticated(&state, &headers) {
        return plain_response(StatusCode::UNAUTHORIZED);
    }
    let path = uri
        .path_and_query()
        .map(|value| value.as_str().to_owned())
        .unwrap_or_else(|| "/ws".into());
    let target = format!("{}{path}", state.ttyd_ws_url);
    let mut request = match target.into_client_request() {
        Ok(request) => request,
        Err(_) => return plain_response(StatusCode::SERVICE_UNAVAILABLE),
    };
    let Ok(host) = HeaderValue::from_str(&state.ttyd_host) else {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(origin) = HeaderValue::from_str(&format!("http://{}", state.ttyd_host)) else {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    };
    request.headers_mut().insert(header::HOST, host);
    request.headers_mut().insert(header::ORIGIN, origin);
    request.headers_mut().insert(
        HeaderName::from_static("x-arqen-auth"),
        HeaderValue::from_static(AUTH_VALUE),
    );
    if let Some(protocols) = headers.get(header::SEC_WEBSOCKET_PROTOCOL) {
        request
            .headers_mut()
            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocols.clone());
    }
    let (upstream, upstream_response) =
        match tokio::time::timeout(Duration::from_secs(5), connect_async(request)).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) | Err(_) => return plain_response(StatusCode::SERVICE_UNAVAILABLE),
        };
    if let Some(protocol) = upstream_response
        .headers()
        .get(header::SEC_WEBSOCKET_PROTOCOL)
    {
        upgrade.set_selected_protocol(protocol.clone());
    } else if headers.contains_key(header::SEC_WEBSOCKET_PROTOCOL) {
        return plain_response(StatusCode::SERVICE_UNAVAILABLE);
    }
    upgrade
        .on_upgrade(move |socket| websocket_proxy(socket, upstream))
        .into_response()
}

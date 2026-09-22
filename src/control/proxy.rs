async fn proxy_http(State(state): State<GatewayState>, request: Request) -> Response {
    if !origin_is_allowed(request.headers()) {
        return plain_response(StatusCode::FORBIDDEN);
    }
    if !is_authenticated(&state, request.headers()) {
        if request.method() == Method::GET && request.uri().path() == "/" {
            return login_response(StatusCode::OK, None);
        }
        return plain_response(StatusCode::UNAUTHORIZED);
    }
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, MAX_PROXY_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => return plain_response(StatusCode::PAYLOAD_TOO_LARGE),
    };
    let path = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target = format!("{}{path}", state.ttyd_http_url);
    let method = match reqwest::Method::from_bytes(parts.method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return plain_response(StatusCode::BAD_REQUEST),
    };
    let mut builder = state
        .client
        .request(method, target)
        .header(AUTH_HEADER, AUTH_VALUE);
    for (name, value) in &parts.headers {
        if should_forward_request_header(name) {
            builder = builder.header(name, value);
        }
    }
    let response = match builder.body(body).send().await {
        Ok(response) => response,
        Err(_) => return plain_response(StatusCode::BAD_GATEWAY),
    };
    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let upstream_headers = response.headers().clone();
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(_) => return plain_response(StatusCode::BAD_GATEWAY),
    };
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    for (name, value) in &upstream_headers {
        if should_forward_response_header(name)
            && let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_ref()),
                HeaderValue::from_bytes(value.as_bytes()),
            )
        {
            response.headers_mut().insert(name, value);
        }
    }
    response
}

fn should_forward_request_header(name: &reqwest::header::HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "host"
            | "cookie"
            | "authorization"
            | "connection"
            | "upgrade"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "content-length"
            | "transfer-encoding"
            | "x-arqen-auth"
    )
}

fn should_forward_response_header(name: &reqwest::header::HeaderName) -> bool {
    !matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "set-cookie"
            | "www-authenticate"
    )
}

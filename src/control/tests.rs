#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::any};
    use std::sync::Mutex as StdMutex;

    fn state(password: &str) -> GatewayState {
        GatewayState::new(password.into(), "127.0.0.1:7682".parse().unwrap()).unwrap()
    }

    fn request(body: &str) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri("/auth/login")
            .header(header::HOST, "127.0.0.1:7681")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[test]
    fn login_page_uses_shared_theme_tokens_and_no_external_assets() {
        let page = login_page(None);
        assert!(page.contains("ARQEN"));
        assert!(page.contains(theme::BACKGROUND_HEX));
        assert!(page.contains(theme::PRIMARY_HEX));
        assert!(page.contains("form method=\"post\" action=\"/auth/login\""));
        assert!(page.contains("placeholder=\"arqen\""));
        assert!(!page.contains("value=\"arqen\""));
        assert!(!page.contains("readonly"));
        assert!(!page.contains("LOCAL SESSION"));
        assert!(page.contains("hidden"));
        assert!(!page.contains("https://"));
    }

    #[test]
    fn secrets_reject_empty_and_control_values() {
        assert!(validate_secret("", "secret").is_err());
        assert!(validate_secret("secret\n", "secret").is_err());
        assert!(validate_secret("secret", "secret").is_ok());
    }

    #[test]
    fn password_file_reader_trims_generator_newline_without_logging_contents() {
        let path = std::env::temp_dir().join(format!(
            "arqen-control-password-test-{}",
            Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"secret\n").unwrap();
        assert_eq!(read_password_file(&path).unwrap(), "secret");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn login_form_rejects_duplicate_credentials() {
        assert!(parse_login_form(b"username=arqen&username=other&password=secret").is_none());
        assert_eq!(
            parse_login_form(b"username=arqen&password=secret"),
            Some(("arqen".into(), "secret".into()))
        );
    }

    #[tokio::test]
    async fn successful_login_sets_session_without_www_authenticate() {
        let state = state("secret");
        let response = login(State(state), request("username=arqen&password=secret")).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(response.headers().get(header::SET_COOKIE).is_some());
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }

    #[tokio::test]
    async fn failed_login_renders_a_safe_error_without_a_session() {
        let state = state("secret");
        let response = login(State(state), request("username=arqen&password=wrong")).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Credentials not recognized."));
    }

    #[tokio::test]
    async fn proxy_requires_session_and_strips_browser_credentials() {
        let seen = Arc::new(StdMutex::new(None::<(Option<String>, bool, bool)>));
        let seen_by_upstream = Arc::clone(&seen);
        let upstream = Router::new().fallback(any(move |request: Request<Body>| {
            let seen = Arc::clone(&seen_by_upstream);
            async move {
                let auth = request
                    .headers()
                    .get(AUTH_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let has_cookie = request.headers().contains_key(header::COOKIE);
                let has_authorization = request.headers().contains_key(header::AUTHORIZATION);
                *seen.lock().unwrap() = Some((auth, has_cookie, has_authorization));
                (StatusCode::OK, "upstream")
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });

        let mut state = state("secret");
        state.ttyd_http_url = format!("http://{upstream_addr}");
        let unauthorized = proxy_http(
            State(state.clone()),
            Request::builder()
                .method(Method::GET)
                .uri("/token")
                .header(header::HOST, "127.0.0.1:7681")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert!(seen.lock().unwrap().is_none());

        let login = login(
            State(state.clone()),
            request("username=arqen&password=secret"),
        )
        .await;
        let cookie = login
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let authorized = proxy_http(
            State(state),
            Request::builder()
                .method(Method::GET)
                .uri("/token")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, cookie)
                .header(header::AUTHORIZATION, "Basic secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(authorized.status(), StatusCode::OK);
        let body = to_bytes(authorized.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"upstream");
        assert_eq!(
            seen.lock().unwrap().as_ref(),
            Some(&(Some(AUTH_VALUE.into()), false, false))
        );
    }

    #[tokio::test]
    async fn proxy_returns_bad_gateway_when_ttyd_is_unavailable() {
        let state = state("secret");
        let token = state.sessions.issue();
        let response = proxy_http(
            State({
                let mut state = state;
                state.ttyd_http_url = "http://127.0.0.1:1".into();
                state
            }),
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, format!("{SESSION_COOKIE}={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[allow(clippy::result_large_err)]
    #[tokio::test]
    async fn websocket_proxy_round_trips_binary_data_and_injects_auth_header() {
        let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        let (auth_sender, auth_receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let upstream = tokio_tungstenite::accept_hdr_async(
                stream,
                move |
                    request: &tokio_tungstenite::tungstenite::handshake::server::Request,
                    mut response: tokio_tungstenite::tungstenite::handshake::server::Response,
                | {
                    let auth = request
                        .headers()
                        .get(AUTH_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let has_cookie = request.headers().contains_key(header::COOKIE);
                    let protocol = request
                        .headers()
                        .get(header::SEC_WEBSOCKET_PROTOCOL)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    if let Some(protocol) = request
                        .headers()
                        .get(header::SEC_WEBSOCKET_PROTOCOL)
                        .cloned()
                    {
                        response
                            .headers_mut()
                            .insert(header::SEC_WEBSOCKET_PROTOCOL, protocol);
                    }
                    let _ = auth_sender.send((auth, has_cookie, protocol));
                    Ok(response)
                },
            )
            .await
            .unwrap();
            let (mut sink, mut stream) = upstream.split();
            if let Some(Ok(message)) = stream.next().await {
                let _ = sink.send(message).await;
            }
        });

        let mut state = state("secret");
        state.ttyd_ws_url = format!("ws://{upstream_addr}");
        state.ttyd_host = upstream_addr.to_string();
        let token = state.sessions.issue();
        let gateway_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway_addr = gateway_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(gateway_listener, build_router(state)).await;
        });

        let mut request = format!("ws://{gateway_addr}/ws")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            header::HOST,
            HeaderValue::from_str(&gateway_addr.to_string()).unwrap(),
        );
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(&format!("http://{gateway_addr}")).unwrap(),
        );
        request.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("tty"),
        );
        let (mut client, response) = connect_async(request).await.unwrap();
        assert_eq!(
            response
                .headers()
                .get(header::SEC_WEBSOCKET_PROTOCOL)
                .and_then(|value| value.to_str().ok()),
            Some("tty")
        );
        let payload = vec![0, 1, 2, 255];
        client
            .send(UpstreamMessage::Binary(payload.clone().into()))
            .await
            .unwrap();
        assert_eq!(
            client.next().await.unwrap().unwrap(),
            UpstreamMessage::Binary(payload.into())
        );
        assert_eq!(
            auth_receiver.await.unwrap(),
            (Some(AUTH_VALUE.into()), false, Some("tty".into()))
        );
    }

    #[tokio::test]
    async fn logout_revokes_session_and_expires_cookie() {
        let state = state("secret");
        let token = state.sessions.issue();
        let response = logout(
            State(state.clone()),
            Request::builder()
                .method(Method::POST)
                .uri("/auth/logout")
                .header(header::HOST, "127.0.0.1:7681")
                .header(header::COOKIE, format!("{SESSION_COOKIE}={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(!state.sessions.contains(&token));
        assert!(
            response
                .headers()
                .get(header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
    }

    #[test]
    fn expired_session_is_removed_on_lookup() {
        let state = state("secret");
        let token = state.sessions.issue();
        state
            .sessions
            .sessions
            .lock()
            .unwrap()
            .insert(token.clone(), Instant::now() - Duration::from_secs(1));
        assert!(!state.sessions.contains(&token));
    }

    #[tokio::test]
    async fn cross_origin_login_is_rejected() {
        let state = state("secret");
        let mut request = request("username=arqen&password=secret");
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("http://unexpected.example"),
        );
        let response = login(State(state), request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn browser_form_with_opaque_origin_is_allowed_only_when_same_origin() {
        let same_origin_state = state("secret");
        let mut same_origin_request = request("username=arqen&password=secret");
        same_origin_request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static("null"));
        same_origin_request.headers_mut().insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        let response = login(State(same_origin_state), same_origin_request).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let cross_site_state = state("secret");
        let mut cross_site_request = request("username=arqen&password=secret");
        cross_site_request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static("null"));
        cross_site_request.headers_mut().insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("cross-site"),
        );
        let response = login(State(cross_site_state), cross_site_request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn failed_login_limiter_blocks_after_five_attempts() {
        let mut limiter = FailedLoginState::default();
        for _ in 0..MAX_FAILED_LOGINS {
            assert!(limiter.retry_after().is_none());
            limiter.record_failure();
        }
        assert!(limiter.retry_after().is_some());
    }

    #[test]
    fn websocket_messages_round_trip_binary_data() {
        let original = AxumMessage::binary(vec![0, 1, 2, 255]);
        let converted = from_upstream_message(to_upstream_message(original.clone())).unwrap();
        assert_eq!(converted, original);
    }
}

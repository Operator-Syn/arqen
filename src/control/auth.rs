fn is_authenticated(state: &GatewayState, headers: &HeaderMap) -> bool {
    session_token(headers).is_some_and(|token| state.sessions.contains(&token))
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE && !value.is_empty()).then(|| value.to_owned())
    })
}

fn session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict",
        SESSION_TTL.as_secs()
    )
}

fn expired_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict")
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if origin == "null" {
        // Chromium uses an opaque Origin for this local form navigation; the
        // Fetch Metadata value keeps opaque cross-site requests rejected.
        return headers
            .get(HeaderName::from_static("sec-fetch-site"))
            .and_then(|value| value.to_str().ok())
            .is_some_and(|site| site.eq_ignore_ascii_case("same-origin"));
    }
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    origin == format!("http://{host}")
}

fn parse_login_form(body: &[u8]) -> Option<(String, String)> {
    let mut username = None;
    let mut password = None;
    for (key, value) in url::form_urlencoded::parse(body) {
        match key.as_ref() {
            "username" if username.is_none() => username = Some(value.into_owned()),
            "password" if password.is_none() => password = Some(value.into_owned()),
            "username" | "password" => return None,
            _ => {}
        }
    }
    Some((username?, password?))
}

fn validate_secret(value: &str, name: &str) -> Result<()> {
    anyhow::ensure!(!value.is_empty(), "{name} cannot be empty");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}

fn read_password_file(path: &std::path::Path) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read control password file at {}", path.display()))?;
    let password = content
        .strip_suffix("\r\n")
        .or_else(|| content.strip_suffix('\n'))
        .unwrap_or(&content)
        .to_owned();
    validate_secret(&password, "control password")?;
    Ok(password)
}

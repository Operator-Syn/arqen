// SPDX-License-Identifier: MPL-2.0
fn login_response(status: StatusCode, error: Option<&str>) -> Response {
    let mut response = Html(login_page(error)).into_response();
    *response.status_mut() = status;
    set_page_headers(response.headers_mut());
    response
}

fn redirect_with_cookie(status: StatusCode, cookie: String) -> Response {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_static("/"));
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    set_page_headers(response.headers_mut());
    response
}

fn plain_response(status: StatusCode) -> Response {
    let mut response = status.into_response();
    set_page_headers(response.headers_mut());
    response
}

fn set_page_headers(headers: &mut HeaderMap) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; img-src data:; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
}

fn login_page(error: Option<&str>) -> String {
    let mut page = LOGIN_PAGE_TEMPLATE.to_owned();
    for (placeholder, value) in [
        ("__BACKGROUND__", theme::BACKGROUND_HEX),
        ("__SURFACE__", theme::SURFACE_HEX),
        ("__SURFACE_RAISED__", theme::SURFACE_RAISED_HEX),
        ("__PRIMARY__", theme::PRIMARY_HEX),
        ("__PRIMARY_STRONG__", theme::PRIMARY_STRONG_HEX),
        ("__TEXT__", theme::TEXT_HEX),
        ("__MUTED__", theme::MUTED_HEX),
        ("__SUCCESS__", theme::SUCCESS_HEX),
        ("__DANGER__", theme::DANGER_HEX),
        ("__BORDER__", theme::BORDER_HEX),
    ] {
        page = page.replace(placeholder, value);
    }
    let error = error.map(escape_html).unwrap_or_default();
    page.replace(
        "__ERROR_HIDDEN__",
        if error.is_empty() { "hidden" } else { "" },
    )
    .replace("__ERROR__", &error)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

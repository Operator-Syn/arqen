#[allow(clippy::too_many_arguments)]
pub(crate) fn render_authorization(
    frame: &mut Frame<'_>,
    area: Rect,
    url: &str,
    compact: bool,
    manual_fallback: bool,
    remote: bool,
    reauthenticate: bool,
    reconnect: bool,
) {
    let spec = authorization_spec(
        url,
        compact,
        manual_fallback,
        remote,
        reauthenticate,
        reconnect,
    );
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_redirect(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: Option<&str>,
    input: &str,
    compact: bool,
) {
    let spec = redirect_spec(notice, input, compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_error(frame: &mut Frame<'_>, area: Rect, message: &str, compact: bool) {
    let spec = error_spec(message, compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_confirm_quit(frame: &mut Frame<'_>, area: Rect, compact: bool) {
    let spec = confirm_quit_spec(compact);
    Modal::render(frame, area, &spec);
}

pub(crate) fn render_disconnect(
    frame: &mut Frame<'_>,
    area: Rect,
    email: &str,
    retry: bool,
    compact: bool,
) {
    let spec = disconnect_spec(email, retry, compact);
    Modal::render(frame, area, &spec);
}

fn browser_candidates(
    url: &str,
    configured_browser: Option<std::ffi::OsString>,
) -> Vec<(std::ffi::OsString, Vec<std::ffi::OsString>)> {
    let mut candidates: Vec<(std::ffi::OsString, Vec<std::ffi::OsString>)> = Vec::new();
    // Prefer the explicitly configured browser. Known browser launchers get a
    // new-window request so the external link is visible and active instead of
    // being silently forwarded to a background tab.
    if let Some(browser) = configured_browser {
        candidates.push((browser.clone(), browser_arguments(&browser, url)));
    }
    // Direct browser fallbacks also request a new window so the browser can
    // activate its window. Desktop URL handlers are retained below as
    // portable fallbacks, but may hand the URL to an existing browser without
    // bringing it to the front.
    for browser in ["brave", "google-chrome-stable", "google-chrome", "chromium"] {
        candidates.push((browser.into(), vec!["--new-window".into(), url.into()]));
    }
    candidates.push(("firefox".into(), vec!["--new-window".into(), url.into()]));
    candidates.push(("gio".into(), vec!["open".into(), url.into()]));
    candidates.push(("xdg-open".into(), vec![url.into()]));
    candidates
}

fn browser_arguments(program: &std::ffi::OsStr, url: &str) -> Vec<std::ffi::OsString> {
    if browser_kind(program).is_some() {
        vec!["--new-window".into(), url.into()]
    } else {
        vec![url.into()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserKind {
    Chromium,
    Firefox,
}

fn browser_kind(program: &OsStr) -> Option<BrowserKind> {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.contains("firefox") {
        Some(BrowserKind::Firefox)
    } else if name.contains("brave") || name.contains("chrome") || name.contains("chromium") {
        Some(BrowserKind::Chromium)
    } else {
        None
    }
}

fn owned_browser_arguments(
    program: &OsStr,
    url: &str,
    profile_dir: &Path,
) -> Option<Vec<OsString>> {
    match browser_kind(program)? {
        BrowserKind::Chromium => {
            let mut profile = OsString::from("--user-data-dir=");
            profile.push(profile_dir.as_os_str());
            Some(vec![
                profile,
                "--no-first-run".into(),
                "--no-default-browser-check".into(),
                "--disable-sync".into(),
                "--new-window".into(),
                url.into(),
            ])
        }
        BrowserKind::Firefox => Some(vec![
            "--no-remote".into(),
            "--profile".into(),
            profile_dir.as_os_str().to_owned(),
            "--new-window".into(),
            url.into(),
        ]),
    }
}

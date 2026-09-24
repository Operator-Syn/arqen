// SPDX-License-Identifier: MPL-2.0
use anyhow::{Context, Result};
use std::{env, fs, path::PathBuf};

const APP_DATA_DIRECTORY: &str = "arqen";
const LEGACY_DATA_DIRECTORY: &str = "google-account-tui";
pub(crate) fn database_path() -> Result<std::path::PathBuf> {
    let base = env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .context("set HOME or XDG_DATA_HOME to choose the data directory")?;
    let directory = base.join(APP_DATA_DIRECTORY);
    let path = directory.join("accounts.sqlite3");
    if path.is_file() {
        return Ok(path);
    }

    let legacy_directory = base.join(LEGACY_DATA_DIRECTORY);
    let legacy_path = legacy_directory.join("accounts.sqlite3");
    if legacy_path.is_file() {
        let legacy_wal = legacy_path.with_extension("sqlite3-wal");
        let legacy_shm = legacy_path.with_extension("sqlite3-shm");
        if !legacy_wal.exists() && !legacy_shm.exists() {
            fs::create_dir_all(&directory)?;
            fs::copy(&legacy_path, &path).with_context(|| {
                format!(
                    "migrate legacy account database from {} to {}",
                    legacy_path.display(),
                    path.display()
                )
            })?;
            return Ok(path);
        }
        return Ok(legacy_path);
    }

    fs::create_dir_all(&directory)?;
    Ok(path)
}

pub(crate) fn config_directory() -> Result<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("set HOME or XDG_CONFIG_HOME to choose the config directory")?;
    Ok(base.join(APP_DATA_DIRECTORY))
}

pub(crate) fn client_secret_path() -> Result<std::path::PathBuf> {
    if let Some(path) = env::var_os("GOOGLE_CLIENT_SECRET") {
        return Ok(std::path::PathBuf::from(path));
    }
    let path = config_directory()?.join("google-client-secret.json");
    if path.is_file() {
        return Ok(path);
    }
    // Keep the repository-local path as a development-only compatibility
    // fallback. Installed services and direct binaries use the XDG path above.
    let path = std::path::PathBuf::from(".secrets/google-client-secret.json");
    anyhow::ensure!(
        path.is_file(),
        "Google OAuth client JSON not found; place it at {} or set GOOGLE_CLIENT_SECRET",
        config_directory()?
            .join("google-client-secret.json")
            .display()
    );
    Ok(path)
}

pub(crate) fn oauth_remote_mode() -> bool {
    matches!(
        env::var("ARQEN_OAUTH_REMOTE").ok().as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

pub(crate) fn oauth_callback_port() -> Result<Option<u16>> {
    if !oauth_remote_mode() {
        return Ok(None);
    }
    let raw = env::var("ARQEN_OAUTH_CALLBACK_PORT").unwrap_or_else(|_| "8765".into());
    let port = raw
        .parse::<u16>()
        .with_context(|| format!("parse ARQEN_OAUTH_CALLBACK_PORT `{raw}`"))?;
    anyhow::ensure!(port != 0, "ARQEN_OAUTH_CALLBACK_PORT cannot be zero");
    Ok(Some(port))
}

pub(crate) fn oauth_callback_bind_addr() -> String {
    env::var("ARQEN_OAUTH_CALLBACK_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".into())
}

pub(crate) fn oauth_callback_public_host() -> String {
    env::var("ARQEN_OAUTH_CALLBACK_PUBLIC_HOST").unwrap_or_else(|_| "127.0.0.1".into())
}

pub(crate) fn configured_broker_socket() -> Result<PathBuf> {
    match env::var_os("ARQEN_GMAIL_BROKER_SOCKET") {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        Some(_) => anyhow::bail!("ARQEN_GMAIL_BROKER_SOCKET cannot be empty"),
        None => arqen::broker::default_socket_path(),
    }
}

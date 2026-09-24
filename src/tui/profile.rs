// SPDX-License-Identifier: MPL-2.0
fn create_browser_profile() -> Result<PathBuf> {
    let base = env::temp_dir();
    for _ in 0..8 {
        let path = base.join(format!("arqen-oauth-{}", uuid::Uuid::new_v4().simple()));
        match fs::create_dir(&path) {
            Ok(()) => {
                if let Err(error) = secure_browser_profile(&path) {
                    let _ = fs::remove_dir_all(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("create temporary browser profile at {}", path.display())
                });
            }
        }
    }
    anyhow::bail!("could not allocate a unique temporary browser profile")
}

fn secure_browser_profile(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("restrict temporary browser profile at {}", path.display()))?;
    }
    Ok(())
}

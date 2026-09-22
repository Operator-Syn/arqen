fn read_credentials(path: &Path) -> Result<InstalledCredentials> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("read OAuth client JSON at {}", path.display()))?;
    let credentials: CredentialsFile = serde_json::from_str(&content)
        .with_context(|| format!("parse OAuth client JSON at {}", path.display()))?;
    Ok(credentials.installed)
}

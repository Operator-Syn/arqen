pub fn run(options: BrokerOptions) -> Result<()> {
    #[cfg(unix)]
    {
        run_unix(options)
    }
    #[cfg(not(unix))]
    {
        let _ = options;
        anyhow::bail!("the credential broker currently requires a Unix host")
    }
}

#[cfg(unix)]
fn run_unix(options: BrokerOptions) -> Result<()> {
    use signal_hook::{
        consts::{SIGINT, SIGTERM},
        flag,
    };
    use std::os::unix::net::UnixListener;

    prepare_socket_path(&options.socket_path)?;
    let listener = UnixListener::bind(&options.socket_path).with_context(|| {
        format!(
            "bind Arqen credential broker socket at {}",
            options.socket_path.display()
        )
    })?;
    restrict_socket_permissions(&options.socket_path)?;
    listener
        .set_nonblocking(true)
        .context("configure credential broker listener")?;
    let owned_socket_identity = socket_identity(&options.socket_path)?;
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGINT, Arc::clone(&shutdown)).context("register broker SIGINT handler")?;
    flag::register(SIGTERM, Arc::clone(&shutdown)).context("register broker SIGTERM handler")?;
    let state = BrokerState {
        database_path: options.database_path,
        credentials_path: options.credentials_path,
        access_tokens: Arc::new(Mutex::new(HashMap::new())),
    };
    let result = loop {
        if shutdown.load(Ordering::Relaxed) {
            break Ok(());
        }
        match listener.accept() {
            Ok(stream) => {
                let (stream, _) = stream;
                let state = state.clone();
                std::thread::Builder::new()
                    .name("arqen-gmail-broker-request".into())
                    .spawn(move || handle_connection(stream, &state))
                    .context("spawn credential broker request")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                break Err(error).context("accept credential broker connection");
            }
        }
    };
    if socket_identity(&options.socket_path).ok() == Some(owned_socket_identity) {
        let _ = fs::remove_file(&options.socket_path);
    }
    result
}

#[cfg(unix)]
fn prepare_socket_path(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create credential broker directory at {}", parent.display()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "restrict credential broker directory at {}",
            parent.display()
        )
    })?;
    if path.exists() {
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::net::UnixStream;

        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect credential broker path at {}", path.display()))?;
        anyhow::ensure!(
            metadata.file_type().is_socket(),
            "credential broker path already exists at {} and is not a Unix socket",
            path.display()
        );
        match UnixStream::connect(path) {
            Ok(_) => anyhow::bail!(
                "credential broker socket already exists at {}; stop the previous broker before starting another",
                path.display()
            ),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) =>
            {
                fs::remove_file(path).with_context(|| {
                    format!(
                        "remove stale credential broker socket at {}",
                        path.display()
                    )
                })?;
            }
            Err(error) => {
                anyhow::bail!(
                    "cannot safely determine whether credential broker socket {} is active: {error}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn socket_identity(path: &Path) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)
        .with_context(|| format!("inspect credential broker socket at {}", path.display()))?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
fn restrict_socket_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("restrict credential broker socket at {}", path.display()))
}

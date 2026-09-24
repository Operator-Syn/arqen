// SPDX-License-Identifier: MPL-2.0
pub(crate) fn run(options: ControlGatewayOptions) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create control gateway runtime")?;
    runtime.block_on(run_async(options))
}

async fn run_async(options: ControlGatewayOptions) -> Result<()> {
    let password = read_password_file(&options.password_file)?;
    let state = GatewayState::new(password, options.ttyd_addr)?;
    let ttyd = Arc::new(tokio::sync::Mutex::new(spawn_ttyd(&options).await?));
    let listener = match tokio::net::TcpListener::bind(options.listen_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            let mut ttyd = ttyd.lock().await;
            let _ = ttyd.kill().await;
            let _ = ttyd.wait().await;
            return Err(error)
                .with_context(|| format!("bind control gateway at {}", options.listen_addr));
        }
    };
    let ttyd_for_shutdown = Arc::clone(&ttyd);
    let ttyd_exited = Arc::new(AtomicBool::new(false));
    let ttyd_exited_by_shutdown = Arc::clone(&ttyd_exited);
    let shutdown = async move {
        tokio::select! {
            _ = shutdown_signal() => {},
            _ = async move {
                let mut ttyd = ttyd_for_shutdown.lock().await;
                let _ = ttyd.wait().await;
                ttyd_exited_by_shutdown.store(true, Ordering::Relaxed);
            } => {},
        }
    };
    let result = axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
        .context("serve Arqen control gateway");
    let mut ttyd = ttyd.lock().await;
    if ttyd.try_wait()?.is_none() {
        let _ = ttyd.kill().await;
        let _ = ttyd.wait().await;
    }
    if ttyd_exited.load(Ordering::Relaxed) && result.is_ok() {
        bail!("ttyd exited before the control gateway stopped")
    }
    result
}

async fn spawn_ttyd(options: &ControlGatewayOptions) -> Result<Child> {
    let port = options.ttyd_addr.port().to_string();
    Command::new(&options.ttyd_path)
        .args(["-W", "-O", "-i", "127.0.0.1", "-p", &port])
        .args(["-t", "rendererType=dom", "-H", AUTH_HEADER])
        .arg(&options.arqen_path)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("start ttyd from {}", options.ttyd_path.display()))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = signal::ctrl_c().await;
    }
}

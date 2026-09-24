// SPDX-License-Identifier: MPL-2.0
impl CallbackServer {
    pub(crate) fn start() -> Result<Self> {
        Self::start_with_port(None, "127.0.0.1", "127.0.0.1")
    }

    #[cfg(test)]
    pub(crate) fn start_on_port(port: u16) -> Result<Self> {
        Self::start_with_port(Some(port), "127.0.0.1", "127.0.0.1")
    }

    pub(crate) fn start_on_port_with_bind(
        port: u16,
        bind_addr: &str,
        public_host: &str,
    ) -> Result<Self> {
        Self::start_with_port(Some(port), bind_addr, public_host)
    }

    fn start_with_port(port: Option<u16>, bind_addr: &str, public_host: &str) -> Result<Self> {
        anyhow::ensure!(
            !bind_addr.is_empty(),
            "OAuth callback bind address cannot be empty"
        );
        anyhow::ensure!(
            !public_host.is_empty(),
            "OAuth callback public host cannot be empty"
        );
        anyhow::ensure!(
            !bind_addr.chars().any(char::is_control) && !public_host.chars().any(char::is_control),
            "OAuth callback address contains control characters"
        );
        let listener = TcpListener::bind((bind_addr, port.unwrap_or(0)))
            .context("bind OAuth callback listener")?;
        listener
            .set_nonblocking(true)
            .context("configure OAuth loopback listener")?;
        let port = listener.local_addr()?.port();
        let base_uri = format!("http://{public_host}:{port}");
        let redirect_uri = format!("{base_uri}/oauth2/callback");
        let token = Uuid::new_v4().simple().to_string();
        let launch_path = format!("{LAUNCH_PATH_PREFIX}{token}");
        let start_path = format!("{START_PATH_PREFIX}{token}");
        let launcher_uri = format!("{base_uri}{launch_path}");
        let (sender, receiver) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let authorization_url = Arc::new(Mutex::new(None));
        let thread_authorization_url = Arc::clone(&authorization_url);
        let helper_flow = Arc::new(AtomicBool::new(false));
        let thread_helper_flow = Arc::clone(&helper_flow);
        let thread_launch_path = launch_path.clone();
        let thread_start_path = start_path.clone();
        let thread = thread::Builder::new()
            .name("arqen-oauth-callback".into())
            .spawn(move || {
                callback_thread(
                    listener,
                    sender,
                    thread_shutdown,
                    thread_launch_path,
                    thread_start_path,
                    thread_authorization_url,
                    thread_helper_flow,
                )
            })
            .context("start OAuth callback listener")?;
        Ok(Self {
            redirect_uri,
            launcher_uri,
            authorization_url,
            receiver,
            shutdown,
            thread: Some(thread),
        })
    }

    pub(crate) fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub(crate) fn launcher_uri(&self) -> &str {
        &self.launcher_uri
    }

    pub(crate) fn set_authorization_url(&self, authorization_url: &str) -> Result<()> {
        anyhow::ensure!(
            !authorization_url
                .bytes()
                .any(|byte| matches!(byte, b'\r' | b'\n')),
            "authorization URL contains invalid control characters"
        );
        let mut stored = self
            .authorization_url
            .lock()
            .map_err(|_| anyhow::anyhow!("authorization URL state is unavailable"))?;
        *stored = Some(authorization_url.to_owned());
        Ok(())
    }

    pub(crate) fn try_receive(&mut self) -> Result<Option<String>> {
        match self.receiver.try_recv() {
            Ok(Ok(target)) => Ok(Some(target)),
            Ok(Err(error)) => bail!(error),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                bail!("OAuth callback listener stopped unexpectedly")
            }
        }
    }
}

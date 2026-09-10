use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(crate) struct CallbackServer {
    redirect_uri: String,
    receiver: Receiver<Result<String, String>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CallbackServer {
    pub(crate) fn start() -> Result<Self> {
        let listener =
            TcpListener::bind(("127.0.0.1", 0)).context("bind OAuth loopback listener")?;
        listener
            .set_nonblocking(true)
            .context("configure OAuth loopback listener")?;
        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://127.0.0.1:{port}/oauth2/callback");
        let (sender, receiver) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("arqen-oauth-callback".into())
            .spawn(move || callback_thread(listener, sender, thread_shutdown))
            .context("start OAuth callback listener")?;
        Ok(Self {
            redirect_uri,
            receiver,
            shutdown,
            thread: Some(thread),
        })
    }

    pub(crate) fn redirect_uri(&self) -> &str {
        &self.redirect_uri
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

impl Drop for CallbackServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn callback_thread(
    listener: TcpListener,
    sender: mpsc::Sender<Result<String, String>>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let result = handle_request(&mut stream);
                let response = callback_page(result.is_ok());
                let _ = write_response(&mut stream, response);
                let _ = sender.send(result.map_err(|error| format!("{error:#}")));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = sender.send(Err(format!("accept OAuth callback request: {error}")));
                break;
            }
        }
    }
}

fn handle_request(stream: &mut TcpStream) -> Result<String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("configure OAuth callback request")?;
    let mut buffer = [0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .context("read OAuth callback request")?;
    let request = std::str::from_utf8(&buffer[..read]).context("decode OAuth callback request")?;
    let target = request
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|line| line.split_whitespace().next())
        .context("OAuth callback request did not contain a GET target")?;
    if !target.starts_with("/oauth2/callback?") {
        bail!("unexpected OAuth callback path")
    }
    Ok(target.to_owned())
}

fn write_response(stream: &mut TcpStream, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .context("write OAuth callback response")?;
    stream.flush().context("flush OAuth callback response")?;
    Ok(())
}

fn callback_page(success: bool) -> &'static str {
    if success {
        "<!doctype html><meta charset=utf-8><title>Arqen login complete</title><script>const closeTab=()=>{try{window.open('', '_self')}catch(_){}try{window.close()}catch(_){} };window.addEventListener('load',closeTab,{once:true});closeTab();setTimeout(closeTab,100);</script><body><h1>Login complete</h1><p>This tab should close automatically. If it remains open, you can close it and return to Arqen.</p></body>"
    } else {
        "<!doctype html><meta charset=utf-8><title>Arqen login error</title><body><h1>Arqen could not complete login</h1><p>You may close this tab and return to Arqen.</p></body>"
    }
}

#[cfg(test)]
mod tests {
    use super::callback_page;

    #[test]
    fn callback_pages_include_close_fallback() {
        let page = callback_page(true);
        assert!(page.contains("window.open('', '_self')"));
        assert!(page.contains("window.close()"));
        assert!(page.contains("This tab should close automatically"));
        assert!(page.contains("If it remains open"));
    }
}

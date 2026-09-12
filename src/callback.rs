use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use uuid::Uuid;

const CALLBACK_PATH_PREFIX: &str = "/oauth2/callback?";
const LAUNCH_PATH_PREFIX: &str = "/oauth2/launch/";
const START_PATH_PREFIX: &str = "/oauth2/start/";

pub(crate) struct CallbackServer {
    redirect_uri: String,
    launcher_uri: String,
    authorization_url: Arc<Mutex<Option<String>>>,
    receiver: Receiver<Result<String, String>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CallbackServer {
    pub(crate) fn start() -> Result<Self> {
        Self::start_with_port(None)
    }

    pub(crate) fn start_on_port(port: u16) -> Result<Self> {
        Self::start_with_port(Some(port))
    }

    fn start_with_port(port: Option<u16>) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))
            .context("bind OAuth loopback listener")?;
        listener
            .set_nonblocking(true)
            .context("configure OAuth loopback listener")?;
        let port = listener.local_addr()?.port();
        let base_uri = format!("http://127.0.0.1:{port}");
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
    launch_path: String,
    start_path: String,
    authorization_url: Arc<Mutex<Option<String>>>,
    helper_flow: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = match handle_request(&mut stream, &launch_path, &start_path) {
                    Ok(request) => request,
                    Err(_error) => {
                        let _ = write_html_response(
                            &mut stream,
                            "400 Bad Request",
                            &error_page("Arqen could not read this browser request."),
                        );
                        continue;
                    }
                };
                match request {
                    CallbackRequest::Launch => {
                        let _ =
                            write_html_response(&mut stream, "200 OK", &launcher_page(&start_path));
                    }
                    CallbackRequest::Start => {
                        let stored_url = authorization_url
                            .lock()
                            .ok()
                            .and_then(|stored| stored.clone());
                        match stored_url {
                            Some(stored_url) => {
                                helper_flow.store(true, Ordering::Relaxed);
                                let _ = write_redirect(&mut stream, &stored_url);
                            }
                            None => {
                                let _ = write_html_response(
                                    &mut stream,
                                    "503 Service Unavailable",
                                    &error_page(
                                        "The login helper is not ready. Return to Arqen and try again.",
                                    ),
                                );
                            }
                        }
                    }
                    CallbackRequest::Callback(target) => {
                        let _ = write_html_response(
                            &mut stream,
                            "200 OK",
                            callback_page(
                                !callback_has_error(&target),
                                helper_flow.load(Ordering::Relaxed),
                            ),
                        );
                        let _ = sender.send(Ok(target));
                        break;
                    }
                    CallbackRequest::Unknown => {
                        let _ = write_html_response(
                            &mut stream,
                            "404 Not Found",
                            &error_page("That Arqen login route was not found."),
                        );
                    }
                }
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

fn callback_has_error(target: &str) -> bool {
    target.split_once('?').is_some_and(|(_, query)| {
        url::form_urlencoded::parse(query.as_bytes()).any(|(key, _)| key == "error")
    })
}

#[derive(Debug, PartialEq, Eq)]
enum CallbackRequest {
    Launch,
    Start,
    Callback(String),
    Unknown,
}

fn handle_request(
    stream: &mut TcpStream,
    launch_path: &str,
    start_path: &str,
) -> Result<CallbackRequest> {
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
    if target == launch_path {
        Ok(CallbackRequest::Launch)
    } else if target == start_path {
        Ok(CallbackRequest::Start)
    } else if target.starts_with(CALLBACK_PATH_PREFIX) {
        Ok(CallbackRequest::Callback(target.to_owned()))
    } else {
        Ok(CallbackRequest::Unknown)
    }
}

fn write_html_response(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .context("write OAuth callback response")?;
    stream.flush().context("flush OAuth callback response")?;
    Ok(())
}

fn write_redirect(stream: &mut TcpStream, location: &str) -> Result<()> {
    anyhow::ensure!(
        !location.bytes().any(|byte| matches!(byte, b'\r' | b'\n')),
        "authorization URL contains invalid control characters"
    );
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(response.as_bytes())
        .context("write OAuth launcher redirect")?;
    stream.flush().context("flush OAuth launcher redirect")?;
    Ok(())
}

fn launcher_page(start_path: &str) -> String {
    let escaped_path = escape_html_attribute(start_path);
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><link rel=icon href=\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E\"><title>Continue to Google</title><style>body{{font:16px system-ui,sans-serif;max-width:42rem;margin:4rem auto;padding:0 1.5rem;line-height:1.5}}button{{font:inherit;padding:.65rem 1rem;cursor:pointer}}#fallback{{display:none}}</style><body><h1>Continue to Google</h1><p>Click the button to open Google sign-in in a new window. Arqen will close that window when login is complete.</p><button id=continue type=button data-start=\"{escaped_path}\">Continue to Google</button><p id=status role=status aria-live=polite></p><p id=fallback><a href=\"{escaped_path}\" target=\"_blank\" rel=\"noopener noreferrer\">Open Google sign-in in a new tab</a></p><noscript><p>JavaScript is disabled. <a href=\"{escaped_path}\" target=\"_blank\" rel=\"noopener noreferrer\">Open Google sign-in in a new tab</a>.</p></noscript><script>const button=document.getElementById('continue');const status=document.getElementById('status');const fallback=document.getElementById('fallback');button.addEventListener('click',()=>{{const popup=window.open(button.dataset.start,'arqen-google-login','popup');if(!popup){{status.textContent='The popup was blocked. Allow popups for this local Arqen page, then try again.';fallback.style.display='block';return;}}try{{popup.opener=null}}catch(_){{}}try{{popup.focus()}}catch(_){{}}status.textContent='Google sign-in opened in a new window. Complete it there; this helper can be closed.';if(window.opener)setTimeout(()=>{{try{{window.close()}}catch(_){{}}}},100);}});</script></body>",
        escaped_path = escaped_path
    )
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn error_page(message: &str) -> String {
    let escaped_message = escape_html_attribute(message);
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><link rel=icon href=\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E\"><title>Arqen login</title><style>:root{{color-scheme:dark}}html,body{{background:#000;color:#fff}}body{{font:16px system-ui,sans-serif;max-width:42rem;margin:3rem auto;padding:0 1.5rem;line-height:1.5}}</style><body><h1>Arqen login</h1><p>{escaped_message}</p></body>"
    )
}

fn callback_page(success: bool, close_after_success: bool) -> &'static str {
    if !success {
        return "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><link rel=icon href=\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E\"><title>Arqen login error</title><style>:root{color-scheme:dark}html,body{background:#000;color:#fff}body{font:16px system-ui,sans-serif;max-width:42rem;margin:3rem auto;padding:0 1.5rem;line-height:1.5}</style><body><h1>Arqen could not complete login</h1><p>You may close this tab and return to Arqen.</p></body>";
    }
    if close_after_success {
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><link rel=icon href=\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E\"><title>Arqen login complete</title><style>:root{color-scheme:dark}html,body{background:#000;color:#fff}body{font:16px system-ui,sans-serif;max-width:42rem;margin:3rem auto;padding:0 1.5rem;line-height:1.5}</style><script>try{window.history.replaceState(null,'','/oauth2/callback')}catch(_){}const closeTab=()=>{try{window.close()}catch(_){}};window.addEventListener('load',closeTab,{once:true});closeTab();setTimeout(closeTab,100);</script><body><h1>Login complete</h1><p>This sign-in window should close automatically.</p></body>"
    } else {
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><link rel=icon href=\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E\"><title>Arqen login complete</title><style>:root{color-scheme:dark}html,body{background:#000;color:#fff}body{font:16px system-ui,sans-serif;max-width:42rem;margin:3rem auto;padding:0 1.5rem;line-height:1.5}</style><script>try{window.history.replaceState(null,'','/oauth2/callback')}catch(_){}const closeTab=()=>{try{window.close()}catch(_){}};</script><body><h1>Login complete</h1><p>Return to Arqen to continue.</p></body>"
    }
}

#[cfg(test)]
mod tests {
    use super::{CallbackServer, callback_page, launcher_page};
    use std::{
        io::{Read, Write},
        net::TcpStream,
        sync::{Mutex as TestMutex, OnceLock},
        thread,
        time::Duration,
    };

    static CALLBACK_TEST_LOCK: OnceLock<TestMutex<()>> = OnceLock::new();

    fn lock_callback_tests() -> std::sync::MutexGuard<'static, ()> {
        CALLBACK_TEST_LOCK
            .get_or_init(|| TestMutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn route_parts(uri: &str) -> (&str, &str) {
        let value = uri.strip_prefix("http://").expect("loopback URI");
        let path_start = value.find('/').expect("loopback URI path");
        (&value[..path_start], &value[path_start..])
    }

    fn request(authority: &str, path: &str) -> String {
        request_with_method(authority, "GET", path)
    }

    fn request_with_method(authority: &str, method: &str, path: &str) -> String {
        for _ in 0..5 {
            let mut stream = TcpStream::connect(authority).expect("connect callback server");
            write!(
                stream,
                "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
            )
            .expect("write callback request");
            let mut response = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => return String::from_utf8(response).expect("UTF-8 callback response"),
                    Ok(read) => response.extend_from_slice(&buffer[..read]),
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof
                        ) && !response.is_empty() =>
                    {
                        return String::from_utf8(response).expect("UTF-8 callback response");
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                    Err(error) => panic!("read callback response: {error}"),
                }
            }
        }
        panic!("callback server reset every test request");
    }

    #[test]
    fn callback_pages_use_seamless_dark_styling() {
        let page = callback_page(true, true);
        assert!(page.contains("history.replaceState"));
        assert!(page.contains("window.close()"));
        assert!(page.contains("This sign-in window should close automatically"));
        assert!(page.contains("background:#000"));
        assert!(page.contains("color:#fff"));
        assert!(!page.contains("<button"));
    }

    #[test]
    fn callback_server_can_bind_a_configured_loopback_port() {
        let _lock = lock_callback_tests();
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let server = CallbackServer::start_on_port(port).expect("callback server");
        assert!(
            server
                .redirect_uri()
                .starts_with(&format!("http://127.0.0.1:{port}/"))
        );
    }

    #[test]
    fn direct_callback_page_redacts_url_without_an_automatic_close() {
        let page = callback_page(true, false);
        assert!(page.contains("history.replaceState"));
        assert!(page.contains("window.close()"));
        assert!(!page.contains("window.addEventListener('load',closeTab"));
        assert!(!page.contains("setTimeout(closeTab"));
        assert!(!page.contains("<button"));
        assert!(page.contains("background:#000"));
        assert!(page.contains("color:#fff"));
        assert!(page.contains("rel=icon href=\"data:image/svg+xml,"));
    }

    #[test]
    fn launcher_page_uses_a_user_activated_popup_and_fallback_link() {
        let page = launcher_page("/oauth2/start/test-token");
        assert!(page.contains("Continue to Google"));
        assert!(page.contains("window.open"));
        assert!(page.contains("popup.opener=null"));
        assert!(page.contains("popup.focus()"));
        assert!(page.contains("if(window.opener)"));
        assert!(page.contains("/oauth2/start/test-token"));
        assert!(page.contains("window.open(button.dataset.start,'arqen-google-login','popup')"));
        assert!(page.contains("The popup was blocked"));
        assert!(page.contains("target=\"_blank\" rel=\"noopener noreferrer\""));
        assert!(!page.contains("https://accounts.google.com"));
    }

    #[test]
    fn callback_server_serves_launcher_redirect_and_callback_in_one_flow() {
        let _lock = lock_callback_tests();
        let mut server = CallbackServer::start().expect("callback server");
        server
            .set_authorization_url("https://accounts.google.com/o/oauth2/v2/auth?state=test")
            .expect("authorization URL");
        let (authority, launch_path) = route_parts(server.launcher_uri());
        let token = launch_path
            .strip_prefix("/oauth2/launch/")
            .expect("launch token");
        let start_path = format!("/oauth2/start/{token}");

        let launch_response = request(authority, launch_path);
        assert!(launch_response.starts_with("HTTP/1.1 200 OK"));
        assert!(launch_response.contains("Cache-Control: no-store"));
        assert!(!launch_response.contains("/oauth2/start/test-token"));
        assert!(launch_response.contains(&start_path));

        let start_response = request(authority, &start_path);
        assert!(start_response.starts_with("HTTP/1.1 302 Found"));
        assert!(
            start_response
                .contains("Location: https://accounts.google.com/o/oauth2/v2/auth?state=test")
        );
        assert!(start_response.contains("Referrer-Policy: no-referrer"));

        let callback_response = request(authority, "/oauth2/callback?code=test-code&state=test");
        assert!(callback_response.starts_with("HTTP/1.1 200 OK"));
        assert!(callback_response.contains("history.replaceState"));

        let target = (0..100).find_map(|_| match server.try_receive().expect("callback result") {
            Some(target) => Some(target),
            None => {
                thread::sleep(Duration::from_millis(5));
                None
            }
        });
        assert_eq!(
            target.as_deref(),
            Some("/oauth2/callback?code=test-code&state=test")
        );
    }

    #[test]
    fn denied_callback_keeps_the_error_page_readable() {
        let _lock = lock_callback_tests();
        let mut server = CallbackServer::start().expect("callback server");
        let (authority, _) = route_parts(server.launcher_uri());
        let response = request(authority, "/oauth2/callback?error=access_denied&state=test");
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Arqen could not complete login"));
        assert!(!response.contains("history.replaceState"));
        let target = (0..100).find_map(|_| match server.try_receive().expect("callback result") {
            Some(target) => Some(target),
            None => {
                thread::sleep(Duration::from_millis(5));
                None
            }
        });
        assert_eq!(
            target.as_deref(),
            Some("/oauth2/callback?error=access_denied&state=test")
        );
    }

    #[test]
    fn unknown_routes_do_not_consume_the_callback_listener() {
        let _lock = lock_callback_tests();
        let server = CallbackServer::start().expect("callback server");
        let (authority, launch_path) = route_parts(server.launcher_uri());
        let unknown_response = request(authority, "/favicon.ico");
        assert!(unknown_response.starts_with("HTTP/1.1 404 Not Found"));
        let malformed_response = request_with_method(authority, "POST", launch_path);
        assert!(malformed_response.starts_with("HTTP/1.1 400 Bad Request"));
        let launch_response = request(authority, launch_path);
        assert!(launch_response.starts_with("HTTP/1.1 200 OK"));
    }

    #[test]
    fn launch_token_and_authorization_url_are_validated() {
        let _lock = lock_callback_tests();
        let server = CallbackServer::start().expect("callback server");
        let (authority, launch_path) = route_parts(server.launcher_uri());
        let invalid_path = format!("{launch_path}x");
        assert!(request(authority, &invalid_path).starts_with("HTTP/1.1 404 Not Found"));
        assert!(
            server
                .set_authorization_url(
                    "https://accounts.google.com\r\nLocation: https://example.test"
                )
                .is_err()
        );
    }

    #[test]
    fn start_route_reports_when_authorization_url_is_not_ready() {
        let _lock = lock_callback_tests();
        let server = CallbackServer::start().expect("callback server");
        let (authority, launch_path) = route_parts(server.launcher_uri());
        let token = launch_path
            .strip_prefix("/oauth2/launch/")
            .expect("launch token");
        let start_response = request(authority, &format!("/oauth2/start/{token}"));
        assert!(start_response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(start_response.contains("login helper is not ready"));
    }
}

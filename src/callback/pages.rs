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

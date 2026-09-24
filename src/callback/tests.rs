// SPDX-License-Identifier: MPL-2.0
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
    fn callback_server_can_bind_container_address_and_advertise_host_loopback() {
        let _lock = lock_callback_tests();
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let server = CallbackServer::start_on_port_with_bind(port, "0.0.0.0", "127.0.0.1")
            .expect("callback server");
        assert_eq!(
            server.redirect_uri(),
            format!("http://127.0.0.1:{port}/oauth2/callback")
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

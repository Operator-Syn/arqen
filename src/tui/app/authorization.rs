impl App {
    fn handle_authorization_key(&mut self, key: KeyEvent) {
        let Screen::Authorization {
            oauth,
            url,
            callback,
            remote,
            intent,
        } = &mut self.screen
        else {
            return;
        };
        match key.code {
            KeyCode::Char('c') => {
                let result = copy_to_clipboard(&mut self.clipboard, url);
                self.notice = Some(match result {
                    Ok(()) => "Authorization URL copied to clipboard.".into(),
                    Err(error) => {
                        self.show_error(
                            "Could not copy authorization URL",
                            format_args!("{error:#}"),
                        );
                        return;
                    }
                });
            }
            KeyCode::Char('o') => {
                if *remote {
                    self.notice = Some(
                        "Remote OAuth mode is active. Open the authorization URL in your local browser while the SSH tunnel remains connected.".into(),
                    );
                    return;
                }
                let using_helper = LOGIN_HELPER_ENABLED && callback.is_some();
                let browser_url = browser_target(callback.as_ref(), url);
                self.browser.take();
                match launch_browser(&browser_url) {
                    Ok(browser) => {
                        let owned = browser.is_some();
                        self.browser = browser;
                        self.notice = Some(if using_helper {
                            "Login helper opened in your browser; click Continue to Google there."
                                .into()
                        } else if owned {
                            "Dedicated Google login window opened; Arqen will close it after login completes."
                                .into()
                        } else {
                            "Google sign-in opened in your browser; complete sign-in there, then return to Arqen."
                                .into()
                        });
                    }
                    Err(error) => {
                        self.show_error(
                            if using_helper {
                                "Could not open login helper"
                            } else {
                                "Could not open authorization URL"
                            },
                            format_args!("{error:#}"),
                        );
                    }
                }
            }
            KeyCode::Enter => {
                if callback.is_some() {
                    self.notice = Some(
                        if *remote {
                            "Open the URL in your local browser; the SSH tunnel will deliver the callback to this TUI."
                        } else if LOGIN_HELPER_ENABLED {
                            "Press [o] to open the login helper."
                        } else {
                            "Press [o] to open Google sign-in, or [c] to copy the URL."
                        }
                        .into(),
                    );
                    return;
                }
                let Some(oauth) = oauth.take() else {
                    self.show_error("Unable to continue login", "authorization state is missing");
                    return;
                };
                let url = std::mem::take(url);
                self.screen = Screen::Redirect {
                    oauth: Some(oauth),
                    input: String::new(),
                    intent: intent.clone(),
                };
                self.notice = Some(format!("Open this URL in your browser:\n{url}"));
            }
            KeyCode::Esc => {
                self.stop_browser();
                self.screen = Screen::Accounts;
            }
            _ => {}
        }
    }
}

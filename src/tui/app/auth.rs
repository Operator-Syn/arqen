// SPDX-License-Identifier: MPL-2.0
impl App {
    fn start_login(&mut self, intent: LoginIntent) {
        self.stop_browser();
        let result = (|| -> Result<Screen> {
            let remote = oauth_remote_mode();
            let callback = match oauth_callback_port()? {
                Some(port) => {
                    let bind_addr = oauth_callback_bind_addr();
                    let public_host = oauth_callback_public_host();
                    Some(
                        callback::CallbackServer::start_on_port_with_bind(
                            port,
                            &bind_addr,
                            &public_host,
                        )
                        .with_context(|| {
                            format!(
                                "bind remote OAuth callback on {public_host}:{port} (listener {bind_addr}); keep the callback route available"
                            )
                        })?,
                    )
                }
                None => callback::CallbackServer::start().ok(),
            };
            let mut oauth = GoogleOAuth::from_file(client_secret_path()?)
                .context("load Google OAuth client configuration")?;
            if let Some(callback) = callback.as_ref() {
                oauth.set_redirect_uri(callback.redirect_uri());
            }
            let url = oauth
                .authorization_url()
                .context("create Google authorization URL")?;
            if LOGIN_HELPER_ENABLED && let Some(callback) = callback.as_ref() {
                callback
                    .set_authorization_url(&url)
                    .context("prepare the Google login helper")?;
            }
            Ok(Screen::Authorization {
                oauth: Some(oauth),
                url,
                callback,
                remote,
                intent,
            })
        })();
        self.screen = match result {
            Ok(screen) => screen,
            Err(error) => {
                self.show_error("Unable to start login", format_args!("{error:#}"));
                return;
            }
        };
    }

    fn finish_login(&mut self, oauth: &mut GoogleOAuth, input: &str, intent: &LoginIntent) {
        let result = (|| -> Result<Account> {
            let expected_subject = match intent {
                LoginIntent::Add => None,
                LoginIntent::Reauthenticate { subject } | LoginIntent::Reconnect { subject } => {
                    Some(subject.as_str())
                }
            };
            let login = oauth
                .finish(
                    parse_callback(input).context("parse pasted Google redirect URL")?,
                    expected_subject,
                )
                .context("complete Google login")?;
            let profile = login.profile;
            let account = Account {
                id: uuid::Uuid::new_v4().to_string(),
                subject: profile.sub.clone(),
                email: profile.email,
                display_name: profile.name,
                token_key: Some(token_key(&profile.sub)),
                granted_scopes: Some(login.granted_scopes),
                connection_state: ConnectionState::Connected,
            };
            self.store
                .upsert_google_account(&account)
                .context("save Google account metadata in SQLite")?;
            Ok(account)
        })();
        self.stop_browser();
        match result {
            Ok(account) => self.complete_login(account),
            Err(error) => self.show_error(
                login_error_context(intent),
                friendly_login_error(intent, &error),
            ),
        }
    }

    fn complete_login(&mut self, _account: Account) {
        match self.reload_accounts() {
            Ok(()) => {}
            Err(error) => {
                self.show_error("Account connected, but refreshing accounts failed", error);
                return;
            }
        }
        self.notice = Some("Google account connected successfully.".into());
        // This is the single success boundary for both automatic and manual
        // callback flows; replacing the screen guarantees the modal is gone
        // on the next frame.
        self.screen = Screen::Accounts;
    }
}

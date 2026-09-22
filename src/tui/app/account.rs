impl App {
    fn confirm_disconnect(&mut self, retry: bool) {
        let Some(account) = self.accounts.get(self.selected) else {
            return;
        };
        let allowed = if retry {
            account.connection_state == ConnectionState::Indeterminate
        } else {
            account.connection_state == ConnectionState::Connected
        };
        if !allowed {
            return;
        }
        self.notice = None;
        self.screen = Screen::ConfirmDisconnect {
            subject: account.subject.clone(),
            email: account.email.clone(),
            retry,
        };
    }

    fn disconnect_account(&mut self, subject: &str) {
        let token_key = self
            .accounts
            .iter()
            .find(|account| account.subject == subject)
            .and_then(|account| account.token_key.clone());
        let result = (|| -> Result<()> {
            // Persist the safe state before touching provider or credential-store state.
            // If the process stops after this point, the next run will not claim
            // that the credential is usable.
            self.store
                .set_connection_state(subject, ConnectionState::Indeterminate)
                .context("mark account connection state as indeterminate")?;
            revoke_google_account(token_key.as_deref(), subject)
                .context("revoke and remove Google credentials")?;
            self.store
                .set_connection_state(subject, ConnectionState::Disconnected)
                .context("save disconnected account state")?;
            Ok(())
        })();
        match result {
            Ok(()) => match self.reload_accounts() {
                Ok(()) => {
                    self.notice = Some("Google account disconnected successfully.".into());
                    self.screen = Screen::Accounts;
                }
                Err(error) => self.show_error(
                    "Account disconnected, but refreshing accounts failed",
                    error,
                ),
            },
            Err(error) => {
                let refresh_error = self.reload_accounts().err();
                if let Some(refresh_error) = refresh_error {
                    self.show_error(
                        "Unable to disconnect account and refresh its state",
                        format_args!("{error:#}\n\n{refresh_error:#}"),
                    );
                } else {
                    self.show_error("Unable to disconnect account", format_args!("{error:#}"));
                }
            }
        }
    }
}

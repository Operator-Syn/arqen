// SPDX-License-Identifier: MPL-2.0
impl App {
    fn handle_key(&mut self, key: KeyEvent) {
        if matches!(self.screen, Screen::Accounts) {
            self.handle_accounts_key(key);
        } else if matches!(self.screen, Screen::Authorization { .. }) {
            self.handle_authorization_key(key);
        } else if matches!(self.screen, Screen::Redirect { .. }) {
            self.handle_redirect_key(key);
        } else if matches!(self.screen, Screen::Error(_)) {
            self.handle_error_key(key);
        } else if matches!(self.screen, Screen::ConfirmDisconnect { .. }) {
            self.handle_disconnect_key(key);
        }
    }

    fn handle_accounts_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                self.pane_focus = match self.pane_focus {
                    PaneFocus::Accounts => PaneFocus::Details,
                    PaneFocus::Details => PaneFocus::Accounts,
                };
            }
            KeyCode::Char('a') => self.start_login(LoginIntent::Add),
            KeyCode::Char('t') => self.toggle_mcp_target(),
            KeyCode::Char('d') => {
                let retry = self.accounts.get(self.selected).is_some_and(|account| {
                    account.connection_state == ConnectionState::Indeterminate
                });
                self.confirm_disconnect(retry);
            }
            KeyCode::Char('l') => {
                if let Some(subject) = self.reconnectable_subject() {
                    self.start_login(LoginIntent::Reconnect { subject });
                }
            }
            KeyCode::Char('r') => {
                if let Some(subject) = self.selected_subject() {
                    self.start_login(LoginIntent::Reauthenticate { subject });
                }
            }
            KeyCode::Up | KeyCode::Char('k') => match self.pane_focus {
                PaneFocus::Accounts => self.move_selection(-1),
                PaneFocus::Details => self.scroll_details(-1),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.pane_focus {
                PaneFocus::Accounts => self.move_selection(1),
                PaneFocus::Details => self.scroll_details(1),
            },
            KeyCode::PageUp => self.move_page(false),
            KeyCode::PageDown => self.move_page(true),
            KeyCode::Home => self.move_home(),
            KeyCode::End => self.move_end(),
            KeyCode::Enter => {
                if let Some(account) = self.accounts.get(self.selected) {
                    self.notice = Some(format!(
                        "Selected {} ({})",
                        account.display_name.as_deref().unwrap_or("Unnamed account"),
                        account.email
                    ));
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {}
            _ => {}
        }
    }
}

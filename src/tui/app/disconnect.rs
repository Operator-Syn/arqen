// SPDX-License-Identifier: MPL-2.0
impl App {
    fn handle_disconnect_key(&mut self, key: KeyEvent) {
        let Screen::ConfirmDisconnect { subject, .. } = &self.screen else {
            return;
        };
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                let subject = subject.clone();
                self.disconnect_account(&subject);
            }
            KeyCode::Esc | KeyCode::Char('n') => {
                self.screen = Screen::Accounts;
                self.notice = Some("Disconnect cancelled.".into());
            }
            _ => {}
        }
    }
}

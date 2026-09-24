// SPDX-License-Identifier: MPL-2.0
impl App {
    fn handle_error_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
            self.screen = Screen::Accounts;
        }
    }
}

impl App {
    fn handle_error_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
            self.screen = Screen::Accounts;
        }
    }
}

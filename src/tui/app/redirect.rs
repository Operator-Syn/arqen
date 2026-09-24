// SPDX-License-Identifier: MPL-2.0
impl App {
    fn handle_redirect_key(&mut self, key: KeyEvent) {
        let Screen::Redirect {
            oauth,
            input,
            intent,
        } = &mut self.screen
        else {
            return;
        };
        match key.code {
            KeyCode::Enter => {
                let input = std::mem::take(input);
                let login_intent = intent.clone();
                let Some(mut oauth) = oauth.take() else {
                    self.show_error("Unable to finish login", "authorization state is missing");
                    return;
                };
                self.finish_login(&mut oauth, &input, &login_intent);
            }
            KeyCode::Esc => {
                self.stop_browser();
                self.screen = Screen::Accounts;
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => input.push(ch),
            _ => {}
        }
    }
}

impl App {

    fn poll_callback(&mut self) {
        let target = match &mut self.screen {
            Screen::Authorization {
                callback: Some(callback),
                ..
            } => match callback.try_receive() {
                Ok(target) => target,
                Err(error) => {
                    self.show_error("OAuth callback failed", error);
                    return;
                }
            },
            _ => None,
        };
        let Some(target) = target else { return };
        let screen = std::mem::replace(&mut self.screen, Screen::Accounts);
        let (mut oauth, redirect_uri, callback, intent) = match screen {
            Screen::Authorization {
                mut oauth,
                callback: Some(callback),
                intent,
                ..
            } => (
                oauth.take(),
                callback.redirect_uri().to_owned(),
                callback,
                intent,
            ),
            _ => return,
        };
        drop(callback);
        let input = format!("{redirect_uri}{target}");
        let Some(mut oauth) = oauth.take() else {
            self.show_error("Unable to finish login", "authorization state is missing");
            return;
        };
        self.finish_login(&mut oauth, &input, &intent);
    }
}

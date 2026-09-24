// SPDX-License-Identifier: MPL-2.0
impl App {
    fn new(store: AccountStore) -> Result<Self> {
        let accounts = store.list_accounts()?;
        let mcp_target_subject = store.mcp_configuration()?.target_google_subject;
        Ok(Self {
            store,
            accounts,
            selected: 0,
            mcp_target_subject,
            pane_focus: PaneFocus::Accounts,
            accounts_scroll: 0,
            details_scroll: 0,
            screen: Screen::Accounts,
            browser: None,
            notice: None,
            clipboard: None,
        })
    }

    fn show_error(&mut self, context: &str, error: impl std::fmt::Display) {
        self.stop_browser();
        self.notice = None;
        self.screen = Screen::Error(format!("{context}\n\n{error}"));
    }

    fn stop_browser(&mut self) {
        self.browser.take();
    }

    fn selected_subject(&self) -> Option<String> {
        self.accounts
            .get(self.selected)
            .map(|account| account.subject.clone())
    }

    fn reconnectable_subject(&self) -> Option<String> {
        self.accounts
            .get(self.selected)
            .filter(|account| {
                matches!(
                    account.connection_state,
                    ConnectionState::Disconnected | ConnectionState::Indeterminate
                )
            })
            .map(|account| account.subject.clone())
    }

    fn reload_accounts(&mut self) -> Result<()> {
        self.accounts = self.store.list_accounts()?;
        self.mcp_target_subject = self.store.mcp_configuration()?.target_google_subject;
        self.selected = self.selected.min(self.accounts.len().saturating_sub(1));
        self.details_scroll = 0;
        Ok(())
    }
}

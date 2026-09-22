impl App {
    fn select_account(&mut self, selected: usize) {
        let selected = selected.min(self.accounts.len().saturating_sub(1));
        if selected != self.selected {
            self.details_scroll = 0;
        }
        self.selected = selected;
    }

    fn toggle_mcp_target(&mut self) {
        let Some(account) = self.accounts.get(self.selected) else {
            return;
        };
        let subject = account.subject.clone();
        let email = account.email.clone();
        if self.mcp_target_subject.as_deref() == Some(subject.as_str()) {
            match self.store.set_mcp_target_subject(None) {
                Ok(()) => {
                    self.mcp_target_subject = None;
                    self.notice = Some("MCP target cleared.".into());
                }
                Err(error) => self.show_error("Could not clear MCP target", error),
            }
            return;
        }
        let Some(reason) = mcp_target_ineligibility(account) else {
            match self.store.set_mcp_target_subject(Some(&subject)) {
                Ok(()) => {
                    self.mcp_target_subject = Some(subject);
                    self.notice = Some(format!("MCP target set to {email}."));
                }
                Err(error) => self.show_error("Could not set MCP target", error),
            }
            return;
        };
        self.notice = Some(format!("Cannot set MCP target: {reason}"));
    }

    fn viewport_rows(&self) -> usize {
        let Ok((width, height)) = crossterm::terminal::size() else {
            return 5;
        };
        ui::pane_viewport(
            ratatui::layout::Rect::new(0, 0, width, height),
            self.accounts.len(),
            self.accounts
                .get(self.selected)
                .map(|account| account.connection_state),
            self.pane_focus,
        )
    }

    fn move_selection(&mut self, delta: isize) {
        if self.accounts.is_empty() {
            return;
        }
        let selected = if delta.is_negative() {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            self.selected
                .saturating_add(delta as usize)
                .min(self.accounts.len().saturating_sub(1))
        };
        self.select_account(selected);
    }

    fn scroll_details(&mut self, delta: isize) {
        if delta.is_negative() {
            self.details_scroll = self.details_scroll.saturating_sub(delta.unsigned_abs());
        } else {
            self.details_scroll = self.details_scroll.saturating_add(delta as usize);
        }
    }

    fn move_page(&mut self, forward: bool) {
        let amount = self.viewport_rows().max(1);
        match self.pane_focus {
            PaneFocus::Accounts => self.move_selection(if forward {
                amount as isize
            } else {
                -(amount as isize)
            }),
            PaneFocus::Details => self.scroll_details(if forward {
                amount as isize
            } else {
                -(amount as isize)
            }),
        }
    }

    fn move_home(&mut self) {
        match self.pane_focus {
            PaneFocus::Accounts => self.select_account(0),
            PaneFocus::Details => self.details_scroll = 0,
        }
    }

    fn move_end(&mut self) {
        match self.pane_focus {
            PaneFocus::Accounts => {
                self.select_account(self.accounts.len().saturating_sub(1));
            }
            PaneFocus::Details => self.details_scroll = usize::MAX,
        }
    }
}

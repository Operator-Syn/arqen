#[cfg(test)]
mod tests {
    use super::{
        InteractionContext, MouseTarget, PaneFocus, UiMode, draw, layout, mouse_target,
        mouse_target_with_focus, pane_at_with_notice, theme,
    };
    use crate::tui::Screen;
    use arqen::{Account, ConnectionState};
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::{Constraint, Layout, Rect},
    };

    fn account(name: &str, email: &str) -> Account {
        Account {
            id: name.to_lowercase(),
            subject: format!("subject-{name}"),
            email: email.into(),
            display_name: Some(name.into()),
            token_key: Some(format!("google/{name}")),
            granted_scopes: Some(vec![
                "email".into(),
                "https://www.googleapis.com/auth/gmail.readonly".into(),
                "openid".into(),
                "profile".into(),
            ]),
            connection_state: ConnectionState::Connected,
        }
    }

    fn rendered(width: u16, height: u16, screen: Screen, accounts: &[Account]) -> String {
        rendered_at(width, height, screen, accounts, 0)
    }

    fn rendered_with_target(
        width: u16,
        height: u16,
        screen: Screen,
        accounts: &[Account],
        target_subject: Option<&str>,
    ) -> String {
        rendered_state_with_target(width, height, screen, accounts, 0, 0, 0, target_subject)
    }

    fn rendered_at(
        width: u16,
        height: u16,
        screen: Screen,
        accounts: &[Account],
        details_scroll: usize,
    ) -> String {
        rendered_state(width, height, screen, accounts, 0, 0, details_scroll)
    }

    fn rendered_state(
        width: u16,
        height: u16,
        screen: Screen,
        accounts: &[Account],
        selected: usize,
        accounts_scroll: usize,
        details_scroll: usize,
    ) -> String {
        rendered_state_with_target(
            width,
            height,
            screen,
            accounts,
            selected,
            accounts_scroll,
            details_scroll,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn rendered_state_with_target(
        width: u16,
        height: u16,
        screen: Screen,
        accounts: &[Account],
        selected: usize,
        accounts_scroll: usize,
        details_scroll: usize,
        target_subject: Option<&str>,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut accounts_scroll = accounts_scroll;
        let mut details_scroll = details_scroll;
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    accounts,
                    selected,
                    target_subject,
                    PaneFocus::Accounts,
                    &mut accounts_scroll,
                    &mut details_scroll,
                    &screen,
                    None,
                )
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn line_containing(output: &str, width: u16, text: &str) -> String {
        output
            .chars()
            .collect::<Vec<_>>()
            .chunks(usize::from(width))
            .map(|line| line.iter().collect::<String>())
            .find(|line| line.contains(text))
            .unwrap_or_else(|| panic!("rendered output does not contain {text:?}: {output}"))
    }

    fn text_column(line: &str, text: &str) -> usize {
        line.find(text)
            .unwrap_or_else(|| panic!("rendered line does not contain {text:?}: {line:?}"))
    }

    #[test]
    fn renders_wide_and_narrow_account_states() {
        let accounts = vec![account("Alex Morgan", "alex@example.com")];
        let wide = rendered(120, 32, Screen::Accounts, &accounts);
        let wide_bottom = rendered_at(120, 32, Screen::Accounts, &accounts, usize::MAX);
        let wide_scroll_frames: Vec<String> = (0..32)
            .map(|offset| rendered_at(120, 32, Screen::Accounts, &accounts, offset))
            .collect();
        let narrow = rendered(80, 24, Screen::Accounts, &accounts);
        let narrow_scroll_frames: Vec<String> = (0..32)
            .map(|offset| rendered_at(80, 24, Screen::Accounts, &accounts, offset))
            .collect();
        let _medium = rendered(100, 24, Screen::Accounts, &accounts);
        let _compact = rendered(60, 18, Screen::Accounts, &accounts);
        let _tiny = rendered(36, 12, Screen::Accounts, &accounts);
        assert!(wide.contains("ARQEN"));
        assert!(!wide.contains("Manage multiple Google accounts"));
        assert!(wide.contains("Connection details"));
        assert!(wide.contains("alex@example.com"));
        assert!(wide.contains("│  Google"));
        assert!(wide.contains(&format!(
            "Provider{}│  Google",
            " ".repeat("Keyring reference".chars().count() - "Provider".chars().count() + 2)
        )));
        assert!(wide.contains("Keyring reference  │  "));
        assert!(wide.contains("Granted scopes"));
        assert!(
            wide_scroll_frames
                .iter()
                .any(|frame| frame.contains("Gmail read-only"))
        );
        assert!(
            wide_scroll_frames
                .iter()
                .any(|frame| frame.contains("https://www.googleapis.com/auth/gmail.readonly"))
        );
        assert!(wide_bottom.contains("Status"));
        assert!(wide_bottom.contains(&format!(
            "Account ID{}│  ",
            " ".repeat("Keyring reference".chars().count() - "Account ID".chars().count() + 2)
        )));
        assert!(wide.contains("[r] reauth"));
        assert!(wide.contains("[q]\u{00a0}quit"));
        assert!(narrow.contains("Selected account"));
        assert!(narrow.contains("alex@example.com"));
        assert!(narrow.contains("Connection details"), "{narrow}");
        assert!(narrow.contains("CONNECTED"), "{narrow}");
        assert!(
            narrow_scroll_frames
                .iter()
                .any(|frame| frame.contains("Additional information"))
        );
        assert!(
            narrow_scroll_frames
                .iter()
                .any(|frame| frame.contains("Gmail"))
        );
        assert!(
            narrow_scroll_frames
                .iter()
                .any(|frame| frame.contains("gmail.readonly"))
        );
    }

    #[test]
    fn detail_section_headings_align_with_table_rows() {
        let accounts = vec![account("Alex Morgan", "alex@example.com")];
        let top = rendered(120, 32, Screen::Accounts, &accounts);
        let bottom = rendered_at(120, 32, Screen::Accounts, &accounts, usize::MAX);

        assert_eq!(
            text_column(
                &line_containing(&top, 120, "Connection details"),
                "Connection details"
            ),
            text_column(&line_containing(&top, 120, "Provider"), "Provider")
        );
        assert_eq!(
            text_column(
                &line_containing(&bottom, 120, "Additional information"),
                "Additional information"
            ),
            text_column(&line_containing(&bottom, 120, "Account ID"), "Account ID")
        );
    }

    #[test]
    fn marks_the_persisted_mcp_target_in_both_panes() {
        let accounts = vec![account("Alex Morgan", "alex@example.com")];
        let output = rendered_with_target(
            120,
            32,
            Screen::Accounts,
            &accounts,
            Some("subject-Alex Morgan"),
        );
        assert!(output.contains("◆"), "{output}");
        assert!(output.contains("MCP target"), "{output}");
        assert!(output.contains("Selected"), "{output}");
    }

    #[test]
    fn renders_persistent_scrollbars_and_reaches_compact_list_rows() {
        let accounts: Vec<Account> = (0..12)
            .map(|index| {
                account(
                    &format!("Account {index}"),
                    &format!("user-{index}@example.com"),
                )
            })
            .collect();
        let output = rendered_state(
            60,
            18,
            Screen::Accounts,
            &accounts,
            accounts.len() - 1,
            usize::MAX,
            usize::MAX,
        );
        assert!(output.contains("│"));
        assert!(output.contains("█"), "{output}");
        assert!(output.contains("user-11@example.com"));
        assert!(output.contains("[Wheel]"));
        assert!(output.contains("scroll"));
        assert!(output.contains("[Tab]"));

        let details_focused =
            rendered_state(120, 32, Screen::Accounts, &accounts[..1], 0, 0, usize::MAX);
        assert!(details_focused.contains("Additional information"));
    }

    #[test]
    fn focus_border_and_scroll_hit_testing_follow_the_active_pane() {
        let area = Rect::new(0, 0, 120, 32);
        let accounts = vec![account("First", "first@example.com")];
        let areas = layout(
            area,
            accounts.len(),
            Some(ConnectionState::Connected),
            PaneFocus::Details,
            None,
        );
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut accounts_scroll = 0;
        let mut details_scroll = 0;
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    &accounts,
                    0,
                    None,
                    PaneFocus::Details,
                    &mut accounts_scroll,
                    &mut details_scroll,
                    &Screen::Accounts,
                    None,
                )
            })
            .unwrap();
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((areas.details.x, areas.details.y))
                .expect("details border cell")
                .fg,
            theme::PRIMARY_STRONG
        );
        let footer_padding = super::content_padding(areas.footer.width);
        let footer_inner = Rect {
            x: areas.footer.x.saturating_add(1 + footer_padding),
            y: areas.footer.y.saturating_add(1),
            width: areas
                .footer
                .width
                .saturating_sub(2 + footer_padding.saturating_mul(2)),
            height: areas.footer.height.saturating_sub(2),
        };
        let footer_columns = Layout::horizontal([
            Constraint::Percentage(60),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(footer_inner);
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((footer_columns[1].x, footer_columns[1].y))
                .expect("footer separator cell")
                .symbol(),
            "│"
        );
        assert_eq!(
            pane_at_with_notice(
                area,
                &accounts,
                0,
                areas.accounts.x + 1,
                areas.accounts.y + 1,
                None,
            ),
            Some(PaneFocus::Accounts)
        );
        assert_eq!(
            pane_at_with_notice(
                area,
                &accounts,
                0,
                areas.details.x + 1,
                areas.details.y + 1,
                None,
            ),
            Some(PaneFocus::Details)
        );
    }

    #[test]
    fn mouse_account_hit_testing_accounts_for_scroll_offset() {
        let area = Rect::new(0, 0, 120, 32);
        let accounts = vec![
            account("First", "first@example.com"),
            account("Second", "second@example.com"),
            account("Third", "third@example.com"),
        ];
        let areas = layout(
            area,
            accounts.len(),
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(
            mouse_target_with_focus(
                area,
                &accounts,
                0,
                areas.accounts.x + 2,
                areas.accounts.y + 4,
                InteractionContext {
                    pane_focus: PaneFocus::Accounts,
                    accounts_scroll: 1,
                },
                None,
            ),
            Some(MouseTarget::Account(1))
        );
    }

    #[test]
    fn scrollbar_offsets_clamp_when_rendered_at_both_ends() {
        let area = Rect::new(0, 0, 80, 24);
        let accounts = vec![account("First", "first@example.com")];
        let backend = TestBackend::new(area.width, area.height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut accounts_scroll = usize::MAX;
        let mut details_scroll = usize::MAX;
        terminal
            .draw(|frame| {
                draw(
                    frame,
                    &accounts,
                    0,
                    None,
                    PaneFocus::Details,
                    &mut accounts_scroll,
                    &mut details_scroll,
                    &Screen::Accounts,
                    None,
                )
            })
            .unwrap();
        assert!(accounts_scroll < usize::MAX);
        assert!(details_scroll < usize::MAX);
    }

    #[test]
    fn renders_unverified_missing_and_unknown_scope_states() {
        let mut unverified = account("Legacy", "legacy@example.com");
        unverified.granted_scopes = None;
        let unverified_output = rendered(180, 40, Screen::Accounts, &[unverified]);
        assert!(unverified_output.contains("Scopes unverified"));
        assert!(unverified_output.contains("Scope evidence"));
        assert!(unverified_output.contains("Unverified"));

        let mut missing_gmail = account("Identity", "identity@example.com");
        missing_gmail.granted_scopes = Some(vec!["openid".into()]);
        let missing_output = rendered(180, 40, Screen::Accounts, &[missing_gmail]);
        assert!(missing_output.contains("Gmail read-only — not granted"));
        assert!(missing_output.contains("Gmail modify — not granted"));

        let mut unknown = account("Unknown", "unknown@example.com");
        unknown.granted_scopes = Some(vec!["https://example.test/future".into()]);
        let unknown_output = rendered(180, 40, Screen::Accounts, &[unknown]);
        assert!(unknown_output.contains("https://example.test/future"));

        let mut canonical_identity = account("Canonical", "canonical@example.com");
        canonical_identity.granted_scopes = Some(vec![
            "https://www.googleapis.com/auth/userinfo.email".into(),
            "https://www.googleapis.com/auth/userinfo.profile".into(),
        ]);
        let canonical_output = rendered(180, 40, Screen::Accounts, &[canonical_identity]);
        assert!(
            canonical_output
                .contains("Email address — https://www.googleapis.com/auth/userinfo.email")
        );
        assert!(
            canonical_output
                .contains("Basic profile — https://www.googleapis.com/auth/userinfo.profile")
        );

        let mut mail_modify = account("Mail modify", "modify@example.com");
        mail_modify.granted_scopes = Some(vec![
            "https://www.googleapis.com/auth/gmail.readonly".into(),
            "https://www.googleapis.com/auth/gmail.modify".into(),
        ]);
        let modify_output = rendered(180, 40, Screen::Accounts, &[mail_modify]);
        assert!(modify_output.contains(
            "Gmail read, compose, and send — https://www.googleapis.com/auth/gmail.modify"
        ));

        let mut disconnected = account("Disconnected", "disconnected@example.com");
        disconnected.connection_state = ConnectionState::Disconnected;
        let disconnected_output = rendered(180, 40, Screen::Accounts, &[disconnected]);
        assert!(disconnected_output.contains("DISCONNECTED"));
        assert!(disconnected_output.contains("Last confirmed scopes"));
        assert!(disconnected_output.contains("Provider grant revoked"));

        let mut indeterminate = account("Unknown state", "unknown-state@example.com");
        indeterminate.connection_state = ConnectionState::Indeterminate;
        let indeterminate_output = rendered(180, 40, Screen::Accounts, &[indeterminate]);
        assert!(indeterminate_output.contains("UNKNOWN"));
        assert!(indeterminate_output.contains("Cleanup incomplete"));
    }

    #[test]
    fn renders_empty_and_dialog_states() {
        let empty = rendered(80, 24, Screen::Accounts, &[]);
        assert!(empty.contains("No accounts yet"));
        let auth = rendered(
            100,
            30,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: None,
                remote: false,
                intent: crate::tui::LoginIntent::Add,
            },
            &[],
        );
        assert!(auth.contains("Connect Google account"));
        let callback = crate::callback::CallbackServer::start().expect("callback server");
        let automatic_auth = rendered(
            120,
            32,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: Some(callback),
                remote: false,
                intent: crate::tui::LoginIntent::Add,
            },
            &[],
        );
        assert!(automatic_auth.contains("Google sign-in"));
        assert!(!automatic_auth.contains("login helper"));
        assert!(automatic_auth.contains("https://accounts.google.com/example"));
        let remote_callback = crate::callback::CallbackServer::start().expect("callback server");
        let remote_auth = rendered(
            120,
            32,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: Some(remote_callback),
                remote: true,
                intent: crate::tui::LoginIntent::Add,
            },
            &[],
        );
        assert!(remote_auth.contains("local browser"));
        assert!(!remote_auth.contains("[o]"));
        let reauth = rendered(
            100,
            30,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: None,
                remote: false,
                intent: crate::tui::LoginIntent::Reauthenticate {
                    subject: "subject".into(),
                },
            },
            &[],
        );
        assert!(reauth.contains("Reauthenticate Google account"));
        let reconnect = rendered(
            100,
            30,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: None,
                remote: false,
                intent: crate::tui::LoginIntent::Reconnect {
                    subject: "subject".into(),
                },
            },
            &[],
        );
        assert!(reconnect.contains("Reconnect Google account"));
        let redirect = rendered(
            100,
            30,
            Screen::Redirect {
                oauth: None,
                input: "http://localhost/?code=example".into(),
                intent: crate::tui::LoginIntent::Add,
            },
            &[],
        );
        assert!(redirect.contains("Finish connection"));
        let error = rendered(100, 30, Screen::Error("Login failed".into()), &[]);
        assert!(error.contains("Login failed"));
        let reauth_error = rendered(
            100,
            30,
            Screen::Error("Reauthentication not completed\n\nTry again".into()),
            &[],
        );
        assert!(reauth_error.contains("Reauthentication not completed"));
        let quit = rendered(100, 30, Screen::ConfirmQuit, &[]);
        assert!(quit.contains("Confirm quit"));
        let disconnect = rendered(
            100,
            30,
            Screen::ConfirmDisconnect {
                subject: "subject".into(),
                email: "user@example.com".into(),
                retry: false,
            },
            &[],
        );
        assert!(disconnect.contains("Disconnect Google account"));
        assert!(disconnect.contains("DISCONNECT"));
    }

    #[test]
    fn mouse_target_matches_account_rows_and_add_action() {
        let area = Rect::new(0, 0, 120, 32);
        let accounts = vec![
            account("First", "first@example.com"),
            account("Second", "second@example.com"),
        ];
        let areas = layout(
            area,
            2,
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.accounts.x + 2,
                areas.accounts.y + 4,
                None,
            ),
            Some(MouseTarget::Account(0))
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.accounts.x + 2,
                areas.accounts.y + 8,
                None,
            ),
            Some(MouseTarget::Account(1))
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.accounts.x + 2,
                areas.accounts.y + 12,
                None,
            ),
            Some(MouseTarget::AddAccount)
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.footer.x + 1 + 9,
                areas.footer.y + 1,
                None,
            ),
            Some(MouseTarget::Disconnect)
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.footer.x + 1 + 25,
                areas.footer.y + 1,
                None,
            ),
            Some(MouseTarget::Reauthenticate)
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.footer.x + 1 + 37,
                areas.footer.y + 1,
                None,
            ),
            Some(MouseTarget::Focus)
        );
        assert_eq!(
            mouse_target(
                area,
                &accounts,
                0,
                areas.details.x + areas.details.width.saturating_sub(5),
                areas.details.y + 4,
                None,
            ),
            Some(MouseTarget::ConnectionBadge)
        );
        assert_eq!(mouse_target(area, &accounts, 0, 0, 0, None), None);

        let compact = Rect::new(0, 0, 60, 24);
        let compact_accounts = vec![
            account("First", "first@example.com"),
            account("Second", "second@example.com"),
            account("Third", "third@example.com"),
        ];
        let compact_areas = layout(
            compact,
            3,
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(compact_areas.mode, UiMode::Compact);
        assert_eq!(
            mouse_target(
                compact,
                &compact_accounts,
                0,
                compact_areas.accounts.x + 1,
                compact_areas.accounts.y + 3,
                None,
            ),
            Some(MouseTarget::Account(0))
        );
        assert_eq!(
            mouse_target_with_focus(
                compact,
                &compact_accounts,
                0,
                compact_areas.accounts.x + 1,
                compact_areas.accounts.y + 4,
                InteractionContext {
                    pane_focus: PaneFocus::Accounts,
                    accounts_scroll: 2,
                },
                None,
            ),
            Some(MouseTarget::AddAccount)
        );

        let empty = Rect::new(0, 0, 80, 24);
        let empty_areas = layout(empty, 0, None, PaneFocus::Accounts, None);
        assert_eq!(
            mouse_target(
                empty,
                &[],
                0,
                empty_areas.accounts.x + 1,
                empty_areas.accounts.y + 1,
                None
            ),
            Some(MouseTarget::AddAccount)
        );
        assert_eq!(
            mouse_target(
                empty,
                &[],
                0,
                empty_areas.details.x + 1,
                empty_areas.details.y + 1,
                None
            ),
            Some(MouseTarget::AddAccount)
        );
    }

    #[test]
    fn layout_scales_columns_and_selects_modes_from_available_space() {
        let wide = layout(
            Rect::new(0, 0, 120, 32),
            3,
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(wide.mode, UiMode::Wide);
        let columns = wide.accounts.width + wide.details.width;
        assert!(wide.accounts.width * 100 >= columns * 35);
        assert!(wide.accounts.width * 100 <= columns * 43);

        let narrow = layout(
            Rect::new(0, 0, 80, 24),
            3,
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(narrow.mode, UiMode::Narrow);
        assert_eq!(narrow.accounts.x, narrow.details.x);
        assert!(narrow.details.y > narrow.accounts.y);

        let compact = layout(
            Rect::new(0, 0, 60, 18),
            3,
            Some(ConnectionState::Connected),
            PaneFocus::Accounts,
            None,
        );
        assert_eq!(compact.mode, UiMode::Compact);
        assert_eq!(compact.accounts.x, compact.details.x);
    }
}

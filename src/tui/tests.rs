#[cfg(test)]
mod tests {
    use super::{App, LoginIntent, PaneFocus, Screen, handle_event, handle_scroll_mouse_at};
    use arqen::{Account, AccountStore, ConnectionState, GMAIL_READONLY_SCOPE};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;
    use std::{
        ffi::{OsStr, OsString},
        path::Path,
    };

    fn app_with_accounts() -> App {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "first".into(),
                subject: "subject-first".into(),
                email: "first@example.com".into(),
                display_name: Some("First Account".into()),
                token_key: Some("google/first".into()),
                granted_scopes: None,
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
        store
            .upsert_google_account(&Account {
                id: "second".into(),
                subject: "subject-second".into(),
                email: "second@example.com".into(),
                display_name: Some("Second Account".into()),
                token_key: Some("google/second".into()),
                granted_scopes: None,
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
        App::new(store).unwrap()
    }

    #[test]
    fn account_selection_is_keyboard_navigable_and_bounded() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn pane_focus_routes_navigation_and_resets_details_on_selection_change() {
        let mut app = app_with_accounts();
        assert_eq!(app.pane_focus, PaneFocus::Accounts);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.pane_focus, PaneFocus::Details);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, 1);
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert!(app.details_scroll > 1);
        app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert!(app.details_scroll < 6);
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, 0);
        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.details_scroll, usize::MAX);

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.pane_focus, PaneFocus::Accounts);
        app.details_scroll = 4;
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        assert_eq!(app.details_scroll, 0);

        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(app.selected, 1);
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn mcp_target_toggle_persists_and_replaces_the_selected_account() {
        let mut app = app_with_accounts();
        for account in &app.accounts {
            app.store
                .upsert_google_account(&Account {
                    granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into()]),
                    ..account.clone()
                })
                .unwrap();
        }
        app.reload_accounts().unwrap();

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject.as_deref(), Some("subject-first"));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("first@example.com"))
        );

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject.as_deref(), Some("subject-second"));
        assert_eq!(
            app.store
                .mcp_configuration()
                .unwrap()
                .target_google_subject
                .as_deref(),
            Some("subject-second")
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject, None);
    }

    #[test]
    fn mcp_target_toggle_explains_ineligible_accounts_without_changing_configuration() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(app.mcp_target_subject, None);
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("granted scopes are unverified"))
        );
    }

    #[test]
    fn mouse_wheel_focuses_and_scrolls_the_pane_under_the_pointer() {
        let mut app = app_with_accounts();
        let area = Rect::new(0, 0, 120, 32);

        handle_scroll_mouse_at(&mut app, area, 2, 8, true);
        assert_eq!(app.pane_focus, PaneFocus::Accounts);
        assert_eq!(app.selected, 1);

        app.selected = 0;
        app.details_scroll = 0;
        handle_scroll_mouse_at(&mut app, area, 70, 8, true);
        assert_eq!(app.pane_focus, PaneFocus::Details);
        assert_eq!(app.details_scroll, 3);
    }

    #[test]
    fn enter_reports_selected_account_and_escape_closes_error() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.notice.as_deref().unwrap().contains("First Account"));
        app.screen = Screen::Error("test error".into());
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.screen, Screen::Accounts));
    }

    #[test]
    fn disconnect_requires_confirmation_and_cancel_preserves_connection() {
        let mut app = app_with_accounts();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(matches!(
            app.screen,
            Screen::ConfirmDisconnect {
                retry: false,
                ref subject,
                ..
            } if subject == "subject-first"
        ));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(app.screen, Screen::Accounts));
        assert_eq!(app.accounts[0].connection_state, ConnectionState::Connected);
    }

    #[test]
    fn indeterminate_disconnect_offers_retry_confirmation() {
        let mut app = app_with_accounts();
        app.store
            .set_connection_state("subject-first", ConnectionState::Indeterminate)
            .unwrap();
        app.reload_accounts().unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert!(matches!(
            app.screen,
            Screen::ConfirmDisconnect { retry: true, .. }
        ));
    }

    #[test]
    fn quitting_requires_confirmation_and_can_be_cancelled() {
        let mut app = app_with_accounts();
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::Accounts));
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            &mut app,
        ));
        assert!(handle_event(
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            &mut app,
        ));
    }

    #[test]
    fn runtime_errors_are_rendered_in_the_error_screen() {
        let mut app = app_with_accounts();
        app.notice = Some("stale notice".into());
        app.show_error("Clipboard failed", "provider unavailable");
        assert!(
            matches!(app.screen, Screen::Error(ref message) if message.contains("Clipboard failed") && message.contains("provider unavailable"))
        );
        assert!(app.notice.is_none());
    }

    #[test]
    fn subject_mismatch_login_error_explains_recovery_without_internal_context() {
        let error = anyhow::anyhow!(
            "complete Google login: {}",
            arqen::auth::SUBJECT_MISMATCH_MESSAGE
        );
        let message = super::friendly_login_error(
            &LoginIntent::Reauthenticate {
                subject: "selected-subject".into(),
            },
            &error,
        );
        assert!(message.contains("different account"));
        assert!(message.contains("No account data or credentials were changed"));
        assert!(!message.contains("complete Google login"));
    }

    #[test]
    fn browser_target_defaults_to_the_direct_authorization_url() {
        let callback = crate::callback::CallbackServer::start().expect("callback server");
        let direct_url = "https://accounts.google.com/o/oauth2/v2/auth?state=test";
        let launcher_url = super::browser_target(Some(&callback), direct_url);
        assert_eq!(launcher_url, direct_url);
        assert_eq!(super::browser_target(None, direct_url), direct_url);
    }

    #[test]
    fn browser_target_can_restore_the_local_launcher_with_the_toggle() {
        let callback = crate::callback::CallbackServer::start().expect("callback server");
        let direct_url = "https://accounts.google.com/o/oauth2/v2/auth?state=test";
        let launcher_url = super::browser_target_with_mode(true, Some(&callback), direct_url);
        assert!(launcher_url.starts_with("http://127.0.0.1:"));
        assert!(launcher_url.contains("/oauth2/launch/"));
        assert_ne!(launcher_url, direct_url);
        assert_eq!(
            super::browser_target_with_mode(false, Some(&callback), direct_url),
            direct_url
        );
    }

    #[test]
    fn browser_candidates_prioritize_foreground_browser_launchers() {
        let candidates = super::browser_candidates("http://127.0.0.1:12345", None);
        let gio_index = candidates
            .iter()
            .position(|(program, _)| program == "gio")
            .expect("gio fallback");
        let brave_index = candidates
            .iter()
            .position(|(program, _)| program == "brave")
            .expect("brave launcher");
        assert!(brave_index < gio_index);
        assert_eq!(candidates[brave_index].1[0], "--new-window");
    }

    #[test]
    fn configured_browser_gets_a_new_window_when_it_is_a_browser_launcher() {
        let candidates = super::browser_candidates(
            "http://127.0.0.1:12345",
            Some("/nix/store/brave-gpu-picker/bin/brave-gpu-picker".into()),
        );
        assert_eq!(
            candidates[0].1,
            vec![
                std::ffi::OsString::from("--new-window"),
                std::ffi::OsString::from("http://127.0.0.1:12345")
            ]
        );
    }

    #[test]
    fn configured_non_browser_launcher_keeps_its_original_argument_shape() {
        let candidates =
            super::browser_candidates("http://127.0.0.1:12345", Some("custom-url-handler".into()));
        assert_eq!(
            candidates[0].1,
            vec![std::ffi::OsString::from("http://127.0.0.1:12345")]
        );
    }

    #[test]
    fn owned_chromium_arguments_use_an_isolated_profile_and_new_window() {
        let arguments = super::owned_browser_arguments(
            OsStr::new("brave-gpu-picker"),
            "http://127.0.0.1:12345",
            Path::new("/tmp/arqen-oauth-test"),
        )
        .expect("Chromium browser arguments");
        assert_eq!(
            arguments[0],
            OsString::from("--user-data-dir=/tmp/arqen-oauth-test")
        );
        assert!(arguments.contains(&OsString::from("--no-first-run")));
        assert!(arguments.contains(&OsString::from("--new-window")));
        assert_eq!(
            arguments.last(),
            Some(&OsString::from("http://127.0.0.1:12345"))
        );
    }

    #[test]
    fn owned_firefox_arguments_disable_remote_reuse() {
        let arguments = super::owned_browser_arguments(
            OsStr::new("firefox"),
            "http://127.0.0.1:12345",
            Path::new("/tmp/arqen-oauth-test"),
        )
        .expect("Firefox browser arguments");
        assert_eq!(arguments[0], OsString::from("--no-remote"));
        assert_eq!(arguments[1], OsString::from("--profile"));
        assert!(arguments.contains(&OsString::from("--new-window")));
        assert_eq!(
            arguments.last(),
            Some(&OsString::from("http://127.0.0.1:12345"))
        );
    }

    #[test]
    fn non_browser_handlers_cannot_be_owned() {
        assert!(
            super::owned_browser_arguments(
                OsStr::new("gio"),
                "http://127.0.0.1:12345",
                Path::new("/tmp/arqen-oauth-test"),
            )
            .is_none()
        );
    }

    #[test]
    fn successful_completion_replaces_any_login_screen() {
        let mut app = app_with_accounts();
        app.screen = Screen::Error("stale login modal".into());
        let account = Account {
            id: "completed".into(),
            subject: "subject-completed".into(),
            email: "completed@example.com".into(),
            display_name: Some("Completed".into()),
            token_key: Some("keyring:arqen:subject-completed".into()),
            granted_scopes: Some(vec!["openid".into()]),
            connection_state: ConnectionState::Connected,
        };
        app.store.upsert_google_account(&account).unwrap();
        app.complete_login(account);
        assert!(matches!(app.screen, Screen::Accounts));
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("successfully"))
        );
        assert!(
            app.accounts
                .iter()
                .any(|account| account.email == "completed@example.com")
        );
    }

    #[test]
    fn ctrl_c_opens_quit_confirmation_from_any_screen() {
        let mut app = app_with_accounts();
        app.screen = Screen::Authorization {
            oauth: None,
            url: "https://accounts.google.com".into(),
            callback: None,
            remote: false,
            intent: LoginIntent::Add,
        };
        assert!(!handle_event(
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            &mut app,
        ));
        assert!(matches!(app.screen, Screen::ConfirmQuit));
    }
}

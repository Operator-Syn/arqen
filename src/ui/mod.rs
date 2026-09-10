mod accounts;
mod chrome;
mod dialogs;
pub(crate) mod modal;
pub(crate) mod theme;

use crate::{PaneFocus, Screen};
use arqen::{Account, ConnectionState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    prelude::Style,
    widgets::Block,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseTarget {
    Account(usize),
    AddAccount,
    Reauthenticate,
    Disconnect,
    Login,
    ConnectionBadge,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiMode {
    Wide,
    Narrow,
    Compact,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct InteractionContext {
    pub(crate) pane_focus: PaneFocus,
    pub(crate) accounts_scroll: usize,
}

pub(crate) fn content_padding(width: u16) -> u16 {
    width.saturating_div(120).clamp(1, 2)
}

#[derive(Debug, Clone, Copy)]
struct Areas {
    header: Rect,
    accounts: Rect,
    details: Rect,
    footer: Rect,
    mode: UiMode,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    frame: &mut Frame<'_>,
    accounts: &[Account],
    selected: usize,
    pane_focus: PaneFocus,
    accounts_scroll: &mut usize,
    details_scroll: &mut usize,
    screen: &Screen,
    notice: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BACKGROUND)),
        area,
    );

    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(area, accounts.len(), selected_state, pane_focus, notice);
    chrome::render_header(frame, areas.header, accounts, areas.mode);
    accounts::render_account_list(
        frame,
        areas.accounts,
        accounts,
        selected,
        areas.mode,
        pane_focus == PaneFocus::Accounts,
        accounts_scroll,
    );
    accounts::render_account_details(
        frame,
        areas.details,
        accounts.get(selected),
        areas.mode,
        pane_focus == PaneFocus::Details,
        details_scroll,
    );
    chrome::render_footer(
        frame,
        areas.footer,
        notice,
        areas.mode,
        selected_state,
        pane_focus,
    );

    match screen {
        Screen::Accounts => {}
        Screen::Authorization {
            url,
            callback,
            intent,
            ..
        } => {
            dialogs::render_authorization(
                frame,
                area,
                url,
                areas.mode == UiMode::Compact,
                callback.is_none(),
                matches!(
                    intent,
                    crate::LoginIntent::Reauthenticate { .. }
                        | crate::LoginIntent::Reconnect { .. }
                ),
                matches!(intent, crate::LoginIntent::Reconnect { .. }),
            );
        }
        Screen::Redirect { input, .. } => {
            dialogs::render_redirect(frame, area, notice, input, areas.mode == UiMode::Compact);
        }
        Screen::Error(message) => {
            dialogs::render_error(frame, area, message, areas.mode == UiMode::Compact);
        }
        Screen::ConfirmQuit => {
            dialogs::render_confirm_quit(frame, area, areas.mode == UiMode::Compact);
        }
        Screen::ConfirmDisconnect { email, retry, .. } => {
            dialogs::render_disconnect(frame, area, email, *retry, areas.mode == UiMode::Compact);
        }
    }
}

#[cfg(test)]
pub(crate) fn mouse_target(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<MouseTarget> {
    mouse_target_with_focus(
        area,
        accounts,
        selected,
        column,
        row,
        InteractionContext {
            pane_focus: PaneFocus::Accounts,
            accounts_scroll: 0,
        },
        notice,
    )
}

pub(crate) fn mouse_target_with_focus(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    context: InteractionContext,
    notice: Option<&str>,
) -> Option<MouseTarget> {
    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(
        area,
        accounts.len(),
        selected_state,
        context.pane_focus,
        notice,
    );
    accounts::mouse_target_with_scroll(
        areas.accounts,
        accounts.len(),
        column,
        row,
        areas.mode,
        context.accounts_scroll,
    )
    .or_else(|| {
        chrome::reauthenticate_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Reauthenticate)
    })
    .or_else(|| {
        chrome::disconnect_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Disconnect)
    })
    .or_else(|| {
        chrome::login_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Login)
    })
    .or_else(|| {
        chrome::focus_target(areas.footer, areas.mode, selected_state, column, row)
            .then_some(MouseTarget::Focus)
    })
    .or_else(|| {
        (selected_state.is_some()
            && accounts::connection_badge_target(areas.details, areas.mode, column, row))
        .then_some(MouseTarget::ConnectionBadge)
    })
    .or_else(|| {
        (accounts.is_empty() && contains(areas.details, column, row))
            .then_some(MouseTarget::AddAccount)
    })
}

pub(crate) fn modal_action(
    area: Rect,
    screen: &Screen,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<modal::ModalActionId> {
    let compact = ui_mode(area) == UiMode::Compact;
    let spec = match screen {
        Screen::Authorization {
            url,
            callback,
            intent,
            ..
        } => dialogs::authorization_spec(
            url,
            compact,
            callback.is_none(),
            matches!(
                intent,
                crate::LoginIntent::Reauthenticate { .. } | crate::LoginIntent::Reconnect { .. }
            ),
            matches!(intent, crate::LoginIntent::Reconnect { .. }),
        ),
        Screen::Redirect { input, .. } => dialogs::redirect_spec(notice, input, compact),
        Screen::Error(message) => dialogs::error_spec(message, compact),
        Screen::ConfirmQuit => dialogs::confirm_quit_spec(compact),
        Screen::ConfirmDisconnect { email, retry, .. } => {
            dialogs::disconnect_spec(email, *retry, compact)
        }
        Screen::Accounts => return None,
    };
    modal::Modal::hit_test(area, &spec, column, row)
}

pub(crate) fn pane_at_with_notice(
    area: Rect,
    accounts: &[Account],
    selected: usize,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<PaneFocus> {
    let selected_state = accounts
        .get(selected)
        .map(|account| account.connection_state);
    let areas = layout(
        area,
        accounts.len(),
        selected_state,
        PaneFocus::Accounts,
        notice,
    );
    if contains(areas.accounts, column, row) {
        Some(PaneFocus::Accounts)
    } else if contains(areas.details, column, row) {
        Some(PaneFocus::Details)
    } else {
        None
    }
}

pub(crate) fn pane_viewport(
    area: Rect,
    account_count: usize,
    selected_state: Option<ConnectionState>,
    pane: PaneFocus,
) -> usize {
    let areas = layout(area, account_count, selected_state, pane, None);
    match pane {
        PaneFocus::Accounts => {
            accounts::list_viewport_rows(areas.accounts, account_count, areas.mode)
        }
        PaneFocus::Details => accounts::details_viewport_rows(areas.details, areas.mode),
    }
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn layout(
    area: Rect,
    _account_count: usize,
    selected_state: Option<ConnectionState>,
    pane_focus: PaneFocus,
    notice: Option<&str>,
) -> Areas {
    let inset = proportional_inset(area);
    let content = Rect {
        x: area.x.saturating_add(inset),
        y: area.y.saturating_add(inset),
        width: area.width.saturating_sub(inset.saturating_mul(2)),
        height: area.height.saturating_sub(inset.saturating_mul(2)),
    };
    let mode = ui_mode(content);
    let vertical = Layout::vertical([
        Constraint::Length(chrome::header_height(content.width, mode)),
        Constraint::Min(1),
        Constraint::Length(chrome::footer_height(
            content.width,
            notice,
            mode,
            selected_state,
            pane_focus,
        )),
    ])
    .split(content);
    let body = if mode == UiMode::Wide {
        Layout::horizontal([
            Constraint::Percentage(39),
            Constraint::Percentage(1),
            Constraint::Fill(1),
        ])
        .split(vertical[1])
    } else {
        Layout::vertical([
            Constraint::Percentage(38),
            Constraint::Length(0),
            Constraint::Fill(1),
        ])
        .split(vertical[1])
    };
    Areas {
        header: vertical[0],
        accounts: body[0],
        details: body[2],
        footer: vertical[2],
        mode,
    }
}

fn proportional_inset(area: Rect) -> u16 {
    let basis = area.width.min(area.height);
    basis.saturating_div(24).clamp(1, 3)
}

fn ui_mode(area: Rect) -> UiMode {
    // Terminal-column breakpoints approximating common 576px/960px web breakpoints.
    if area.height < 20 || area.width < 72 {
        UiMode::Compact
    } else if area.width < 118 {
        UiMode::Narrow
    } else if u32::from(area.width) * 10 >= u32::from(area.height) * 37 {
        UiMode::Wide
    } else {
        UiMode::Narrow
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InteractionContext, MouseTarget, PaneFocus, UiMode, draw, layout, mouse_target,
        mouse_target_with_focus, pane_at_with_notice, theme,
    };
    use crate::Screen;
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
                intent: crate::LoginIntent::Add,
            },
            &[],
        );
        assert!(auth.contains("Connect Google account"));
        let reauth = rendered(
            100,
            30,
            Screen::Authorization {
                oauth: None,
                url: "https://accounts.google.com/example".into(),
                callback: None,
                intent: crate::LoginIntent::Reauthenticate {
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
                intent: crate::LoginIntent::Reconnect {
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
                intent: crate::LoginIntent::Add,
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

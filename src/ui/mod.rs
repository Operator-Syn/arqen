mod accounts;
mod chrome;
mod dialogs;
pub(crate) mod modal;
pub(crate) mod theme;

use crate::Screen;
use arqen::Account;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiMode {
    Wide,
    Narrow,
    Compact,
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

pub(crate) fn draw(
    frame: &mut Frame<'_>,
    accounts: &[Account],
    selected: usize,
    screen: &Screen,
    notice: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::BACKGROUND)),
        area,
    );

    let areas = layout(area, accounts.len(), notice);
    chrome::render_header(frame, areas.header, accounts, areas.mode);
    accounts::render_account_list(frame, areas.accounts, accounts, selected, areas.mode);
    accounts::render_account_details(frame, areas.details, accounts.get(selected), areas.mode);
    chrome::render_footer(frame, areas.footer, notice, areas.mode);

    match screen {
        Screen::Accounts => {}
        Screen::Authorization { url, callback, .. } => {
            dialogs::render_authorization(
                frame,
                area,
                url,
                areas.mode == UiMode::Compact,
                callback.is_none(),
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
    }
}

pub(crate) fn mouse_target(
    area: Rect,
    account_count: usize,
    column: u16,
    row: u16,
    notice: Option<&str>,
) -> Option<MouseTarget> {
    let areas = layout(area, account_count, notice);
    accounts::mouse_target(areas.accounts, account_count, column, row, areas.mode).or_else(|| {
        (account_count == 0 && contains(areas.details, column, row))
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
        Screen::Authorization { url, callback, .. } => {
            dialogs::authorization_spec(url, compact, callback.is_none())
        }
        Screen::Redirect { input, .. } => dialogs::redirect_spec(notice, input, compact),
        Screen::Error(message) => dialogs::error_spec(message, compact),
        Screen::ConfirmQuit => dialogs::confirm_quit_spec(compact),
        Screen::Accounts => return None,
    };
    modal::Modal::hit_test(area, &spec, column, row)
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn layout(area: Rect, account_count: usize, notice: Option<&str>) -> Areas {
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
        Constraint::Length(chrome::footer_height(content.width, notice, mode)),
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
            Constraint::Percentage(if account_count == 0 { 34 } else { 38 }),
            Constraint::Percentage(2),
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
    use super::{MouseTarget, UiMode, draw, layout, mouse_target};
    use crate::Screen;
    use arqen::Account;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn account(name: &str, email: &str) -> Account {
        Account {
            id: name.to_lowercase(),
            subject: format!("subject-{name}"),
            email: email.into(),
            display_name: Some(name.into()),
            token_key: Some(format!("google/{name}")),
        }
    }

    fn rendered(width: u16, height: u16, screen: Screen, accounts: &[Account]) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw(frame, accounts, 0, &screen, None))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn renders_wide_and_narrow_account_states() {
        let accounts = vec![account("Alex Morgan", "alex@example.com")];
        let wide = rendered(120, 32, Screen::Accounts, &accounts);
        let narrow = rendered(80, 24, Screen::Accounts, &accounts);
        let _medium = rendered(100, 24, Screen::Accounts, &accounts);
        let _compact = rendered(60, 18, Screen::Accounts, &accounts);
        let _tiny = rendered(36, 12, Screen::Accounts, &accounts);
        assert!(wide.contains("ARQEN"));
        assert!(!wide.contains("Manage multiple Google accounts"));
        assert!(wide.contains("Connection details"));
        assert!(wide.contains("alex@example.com"));
        assert!(wide.contains("Gmail read-only"));
        assert!(wide.contains("Scopes"));
        assert!(wide.contains("Additional information"));
        assert!(narrow.contains("Selected account"));
        assert!(narrow.contains("alex@example.com"));
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
            },
            &[],
        );
        assert!(auth.contains("Connect Google account"));
        let redirect = rendered(
            100,
            30,
            Screen::Redirect {
                oauth: None,
                input: "http://localhost/?code=example".into(),
            },
            &[],
        );
        assert!(redirect.contains("Finish connection"));
        let error = rendered(100, 30, Screen::Error("Login failed".into()), &[]);
        assert!(error.contains("Login failed"));
        let quit = rendered(100, 30, Screen::ConfirmQuit, &[]);
        assert!(quit.contains("Confirm quit"));
    }

    #[test]
    fn mouse_target_matches_account_rows_and_add_action() {
        let area = Rect::new(0, 0, 120, 32);
        let areas = layout(area, 2, None);
        assert_eq!(
            mouse_target(area, 2, areas.accounts.x + 2, areas.accounts.y + 4, None),
            Some(MouseTarget::Account(0))
        );
        assert_eq!(
            mouse_target(area, 2, areas.accounts.x + 2, areas.accounts.y + 8, None),
            Some(MouseTarget::Account(1))
        );
        assert_eq!(
            mouse_target(area, 2, areas.accounts.x + 2, areas.accounts.y + 12, None),
            Some(MouseTarget::AddAccount)
        );
        assert_eq!(mouse_target(area, 2, 0, 0, None), None);

        let compact = Rect::new(0, 0, 60, 24);
        let compact_areas = layout(compact, 3, None);
        assert_eq!(compact_areas.mode, UiMode::Compact);
        assert_eq!(
            mouse_target(
                compact,
                3,
                compact_areas.accounts.x + 1,
                compact_areas.accounts.y + 3,
                None,
            ),
            Some(MouseTarget::Account(0))
        );
        assert_eq!(
            mouse_target(
                compact,
                3,
                compact_areas.accounts.x + 1,
                compact_areas.accounts.y + 6,
                None,
            ),
            Some(MouseTarget::AddAccount)
        );

        let empty = Rect::new(0, 0, 80, 24);
        let empty_areas = layout(empty, 0, None);
        assert_eq!(
            mouse_target(
                empty,
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
        let wide = layout(Rect::new(0, 0, 120, 32), 3, None);
        assert_eq!(wide.mode, UiMode::Wide);
        let columns = wide.accounts.width + wide.details.width;
        assert!(wide.accounts.width * 100 >= columns * 35);
        assert!(wide.accounts.width * 100 <= columns * 43);

        let narrow = layout(Rect::new(0, 0, 80, 24), 3, None);
        assert_eq!(narrow.mode, UiMode::Narrow);
        assert_eq!(narrow.accounts.x, narrow.details.x);
        assert!(narrow.details.y > narrow.accounts.y);

        let compact = layout(Rect::new(0, 0, 60, 18), 3, None);
        assert_eq!(compact.mode, UiMode::Compact);
        assert_eq!(compact.accounts.x, compact.details.x);
    }
}

mod accounts;
mod chrome;
mod dialogs;
pub(crate) mod modal;
pub(crate) mod theme;

use crate::tui::{LoginIntent, PaneFocus, Screen};
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

include!("render.rs");
include!("interaction.rs");
include!("layout.rs");
include!("tests.rs");

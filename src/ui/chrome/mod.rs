// SPDX-License-Identifier: MPL-2.0
use super::{PaneFocus, UiMode, theme};
use arqen::{Account, ConnectionState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::{Line, Span, Style, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

mod footer;
mod header;

pub(crate) use footer::*;
pub(crate) use header::*;
#[cfg(test)]
#[path = "../../../tests/unit/ui/chrome.rs"]
mod tests;

// SPDX-License-Identifier: MPL-2.0
use super::{UiMode, theme};
use arqen::{Account, ConnectionState, GMAIL_MODIFY_SCOPE, GMAIL_READONLY_SCOPE};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Alignment, Line, Span, Style},
    widgets::{
        Block, Borders, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

const EMAIL_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.email";
const PROFILE_SCOPE: &str = "https://www.googleapis.com/auth/userinfo.profile";
const CONNECTION_BADGE_WIDTH: u16 = 21;
const SCROLLBAR_TRACK_WIDTH: u16 = 1;
const SCROLLBAR_PADDING: u16 = 1;
const SCROLLBAR_RESERVED_WIDTH: u16 = SCROLLBAR_TRACK_WIDTH + SCROLLBAR_PADDING;
const DETAIL_COLUMN_GAP: usize = 2;

include!("status.rs");
include!("list.rs");
include!("details.rs");
include!("scroll.rs");
include!("interaction.rs");

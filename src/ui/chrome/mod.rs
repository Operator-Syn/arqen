use super::{PaneFocus, UiMode, theme};
use arqen::{Account, ConnectionState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    prelude::{Line, Span, Style, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

include!("header.rs");
include!("footer.rs");
include!("tests.rs");

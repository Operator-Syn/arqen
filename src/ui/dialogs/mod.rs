// SPDX-License-Identifier: MPL-2.0
use super::modal::{ActionTone, Modal, ModalAction, ModalActionId, ModalSpec, ModalTone};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    prelude::{Line, Span, Text},
};

include!("authorization.rs");
include!("redirect.rs");
include!("status.rs");

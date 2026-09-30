// SPDX-License-Identifier: MPL-2.0
use super::modal::{ActionTone, Modal, ModalAction, ModalActionId, ModalSpec, ModalTone};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    prelude::{Line, Span, Text},
};

mod authorization;
mod redirect;
mod status;

pub(crate) use authorization::*;
pub(crate) use redirect::*;
pub(crate) use status::*;

// SPDX-License-Identifier: MPL-2.0
use super::theme;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    prelude::{Line, Span, Style, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalTone {
    Neutral,
    Warning,
    Danger,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionTone {
    Primary,
    Danger,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalActionId {
    Confirm,
    Cancel,
    Copy,
    Open,
    Continue,
    Close,
}

pub(crate) struct ModalAction {
    pub id: ModalActionId,
    pub label: String,
    pub shortcut: String,
    pub tone: ActionTone,
}

pub(crate) struct ModalSpec {
    pub title: String,
    pub tone: ModalTone,
    pub body: Text<'static>,
    pub actions: Vec<ModalAction>,
    pub focused_action: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ModalLayout {
    pub modal: Rect,
    pub body: Rect,
    pub actions: Vec<Rect>,
}

pub(crate) struct Modal;

include!("render.rs");
include!("layout.rs");
include!("tests.rs");

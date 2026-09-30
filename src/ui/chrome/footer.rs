// SPDX-License-Identifier: MPL-2.0
use super::*;

#[path = "footer_actions.rs"]
mod footer_actions;
#[path = "footer_notice.rs"]
mod footer_notice;
#[path = "footer_render.rs"]
mod footer_render;
#[path = "footer_targets.rs"]
mod footer_targets;

pub(crate) use footer_actions::*;
pub(crate) use footer_notice::*;
pub(crate) use footer_render::*;
pub(crate) use footer_targets::*;

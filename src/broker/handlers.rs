// SPDX-License-Identifier: MPL-2.0
use super::*;

mod deletion;
mod emails;
mod errors;
mod labels;
mod target;
mod tokens;

pub(super) use deletion::{handle_delete_marked_email, handle_mark_email_for_deletion};
pub(super) use emails::{
    handle_list_emails, handle_mark_email_read, handle_mark_email_unread, handle_read_email,
};
pub(super) use errors::*;
pub(super) use labels::{
    handle_apply_label, handle_create_label, handle_delete_label, handle_list_labels,
};
pub(super) use target::*;
use tokens::*;

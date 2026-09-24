// SPDX-License-Identifier: MPL-2.0
pub mod auth;
pub mod broker;
pub mod gmail;
pub mod mcp;
pub mod secrets;
mod store;

pub use store::{Account, AccountStore, ConnectionState, McpConfiguration};

pub const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";
pub const GMAIL_MODIFY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.modify";

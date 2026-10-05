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
#[cfg(test)]
extern crate self as arqen;
#[cfg(test)]
#[path = "../tests/unit/bulk_fixture.rs"]
pub(crate) mod bulk_fixture;
#[cfg(test)]
#[allow(dead_code)]
mod config;
// Compile the real HTTP boundary into library-only integration tests without
// exposing broker internals or a provider override in production builds.
#[cfg(test)]
#[allow(dead_code, unused_imports)]
#[path = "server/mod.rs"]
pub(crate) mod server_test_support;

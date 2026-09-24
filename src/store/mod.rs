// SPDX-License-Identifier: MPL-2.0
use anyhow::Result;
use rusqlite::{Connection, params};

pub const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub subject: String,
    pub email: String,
    pub display_name: Option<String>,
    /// Opaque refresh-token-store reference; never the token itself.
    pub token_key: Option<String>,
    /// The exact scope set returned by Google for the last successful login.
    /// `None` means this account predates scope tracking or has no verified grant.
    pub granted_scopes: Option<Vec<String>>,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Indeterminate,
}

impl ConnectionState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Indeterminate => "indeterminate",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "connected" => Ok(Self::Connected),
            "disconnected" => Ok(Self::Disconnected),
            "indeterminate" => Ok(Self::Indeterminate),
            _ => anyhow::bail!("unknown account connection state: {value}"),
        }
    }
}

pub struct AccountStore {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfiguration {
    pub target_google_subject: Option<String>,
}

include!("schema.rs");
include!("accounts.rs");
include!("target.rs");
include!("queries.rs");
include!("tests.rs");

// SPDX-License-Identifier: MPL-2.0
//! Refresh-token storage backends.
//!
//! Native runs keep using the OS keyring. Docker runs select the OpenBao
//! backend with `ARQEN_SECRET_BACKEND=openbao`; the MCP process never talks to
//! either backend because it only talks to the credential broker.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{env, fs, path::Path};

const KEYRING_SERVICE: &str = "arqen";
const DEFAULT_OPENBAO_ADDR: &str = "http://openbao:8200";
const DEFAULT_OPENBAO_MOUNT: &str = "secret";
const DEFAULT_OPENBAO_PREFIX: &str = "arqen/google";
pub const OPENBAO_REFERENCE_PREFIX: &str = "openbao:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Keyring,
    OpenBao,
}

mod backend;
mod config;
#[path = "keyring.rs"]
mod keyring_backend;
mod openbao;

pub use backend::token_reference;
use backend::*;
pub(crate) use backend::{delete_refresh_token, load_refresh_token, store_refresh_token};
use config::*;
use keyring_backend::*;
pub(crate) use openbao::OpenBaoClient;
pub use openbao::OpenBaoFailure;
#[cfg(test)]
#[path = "../../tests/unit/secrets.rs"]
mod tests;

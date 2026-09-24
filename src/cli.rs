// SPDX-License-Identifier: MPL-2.0
use anyhow::{Context, Result};
use std::env;

pub(crate) fn run() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("credential-broker") => run_credential_broker(),
        Some("control-gateway") => run_control_gateway(),
        Some("mcp-server") => run_mcp_server(),
        Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some(command) => anyhow::bail!("unknown Arqen command `{command}`; use --help"),
        None => crate::tui::run(),
    }
}

fn run_credential_broker() -> Result<()> {
    let socket_path = crate::config::configured_broker_socket()?;
    let database_path =
        crate::config::database_path().context("open application data directory")?;
    let credentials_path = crate::config::client_secret_path()?;
    arqen::broker::run(arqen::broker::BrokerOptions {
        socket_path,
        database_path,
        credentials_path,
    })
}

fn run_mcp_server() -> Result<()> {
    let options =
        crate::server::ServerOptions::from_env(crate::config::configured_broker_socket()?)?;
    crate::server::run(options)
}

fn run_control_gateway() -> Result<()> {
    let options = crate::control::ControlGatewayOptions::from_env()?;
    crate::control::run(options)
}

fn print_help() {
    println!(
        "Arqen\n\nCommands:\n  credential-broker  Serve protected-store-backed Gmail access over a Unix socket\n  control-gateway    Serve the branded local TUI sign-in gateway\n  mcp-server         Serve the Streamable HTTP MCP endpoint\n\nWith no command, start the interactive account TUI."
    );
}

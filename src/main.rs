// SPDX-License-Identifier: MPL-2.0
mod callback;
mod cli;
mod config;
mod control;
mod server;
mod tui;
mod ui;

fn main() -> anyhow::Result<()> {
    cli::run()
}

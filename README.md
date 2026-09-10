# Arqen

An agent-agnostic Rust TUI for authenticating and managing multiple Google
accounts. Agents, MCP servers, and other applications can consume the account
metadata and credentials later.

## Current slice

- SQLite account metadata store with migrations.
- Multiple accounts are supported.
- Re-authentication upserts by Google's stable `sub`/subject identifier.
- Press `a` in the TUI to start Google login.
- Login displays a Google authorization URL, captures the Google loopback
  redirect automatically, exchanges the authorization code, stores the refresh token in the OS keyring,
  and saves only account metadata plus a key reference in SQLite.
- Press `q` or `Esc` to exit.

The database is created at `$XDG_DATA_HOME/arqen/accounts.sqlite3`, or
`~/.local/share/arqen/accounts.sqlite3` when `XDG_DATA_HOME` is unset. Existing
databases from the former `google-account-tui` path are migrated without
deleting the legacy copy; databases with active SQLite WAL sidecars continue to
be read from their legacy path safely.

## Run

The app automatically reads `.secrets/google-client-secret.json`. To use a
different location, set `GOOGLE_CLIENT_SECRET` before starting:

```bash
export GOOGLE_CLIENT_SECRET="/path/to/client_secret.json"
```

Start it with:

```bash
cargo run
```

Then:

1. Press `a`.
2. Open the displayed URL in a browser.
3. Choose the Google account and approve only the permissions you want to apply.
4. Arqen captures the loopback redirect and completes the login automatically.
5. If the loopback listener cannot start, the TUI provides the legacy manual
   redirect-input fallback.

The initial scope includes OpenID profile/email identity and Gmail read-only
access because the configured Google project enables Gmail integration. The
TUI itself is not coupled to Hermes or any particular agent.

## Security boundary

SQLite stores account metadata and a keyring reference. It does not store OAuth
access or refresh tokens. Refresh tokens are stored using the `keyring` crate,
which uses the Linux Secret Service backend where available. Access tokens are
held only during the login exchange.

New logins use Arqen's keyring namespace. Existing SQLite rows retain their
legacy keyring references so previously stored credentials are not deleted or
orphaned by the rename.

The OAuth desktop client JSON remains outside this repository. Do not commit it.

## Development

Use the repository's pinned Nix environment (Rust 1.97+) before running the
commands below. With direnv enabled this happens automatically:

```bash
nix develop .#arqen
```

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build
```

### Repository code knowledge base

This repository includes a project-scoped `codebase-memory-mcp` knowledge graph
for symbol search, call-path tracing, architecture discovery, and impact
analysis. It is registered over stdio in `.mcp.json` and `.codex/config.toml`;
neither configuration changes a global agent setup.

Docker is required. Initial setup needs network access to build the pinned
image and download its verified `codebase-memory-mcp@0.10.8` runtime:

```bash
bash .codex/mcp/codebase-memory/setup.sh
```

The setup enables automatic indexing and watching and performs the first index.
After setup, restart or reconnect the MCP client so it reads the project
registration. Runtime MCP sessions use Docker with networking disabled, mount
only this repository as read-only, and keep their writable SQLite graph,
configuration, logs, and runtime files under the ignored
`.codex/mcp/codebase-memory/state/` directory. The server is restricted to the
container path `/workspace/project`; the graph is not committed or shared with
other checkouts.

The complete 15-tool server surface is available. Indexing, project deletion,
ADR updates, and trace ingestion mutate only the private graph state and require
approval in the project Codex configuration. They cannot write to the source
mount. No HTTP graph UI or port is enabled.

Verify the built image, stdio protocol, tool list, representative queries, and
repository boundary with:

```bash
bash .codex/mcp/codebase-memory/test.sh
bash .codex/mcp/codebase-memory/run.sh cli list_projects
bash .codex/mcp/codebase-memory/run.sh cli index_status --project workspace-project
```

A successful build proves the pinned image can be produced. The focused test
also exercises the local stdio server and current index; it does not configure
another checkout, modify global settings, or deploy a service.

### Git hooks

The repository includes a versioned pre-commit hook that requires every commit
to contain exactly one staged path. It rejects empty and multi-path commits,
including rename changes that resolve to multiple path names. The hook does not
stage, unstage, or modify files for you.

Configure it once per checkout:

```bash
bash .githooks/setup.sh
```

This writes only the local `.git/config` setting `core.hooksPath=.githooks`;
there is no global Git configuration change. The hook runs before normal
commits, including commits containing a deletion. Do not bypass it with
`git commit --no-verify` when the one-file commit policy is required.

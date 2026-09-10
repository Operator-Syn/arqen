# Arqen

An agent-agnostic Rust TUI for authenticating and managing multiple Google
accounts. Agents, MCP servers, and other applications can consume the account
metadata and credentials later.

## Current slice

- SQLite account metadata store with migrations.
- Multiple accounts are supported.
- Re-authentication upserts by Google's stable `sub`/subject identifier.
- Press `a` in the TUI to start Google login.
- Press `d` with a connected account selected to confirm Google logout. Arqen
  revokes that grant and removes the local refresh token while keeping the
  identity card for reconnecting.
- Press `l` with a disconnected account selected to reconnect it through the
  same Google login flow used for adding an account.
- Press `r` with an account selected to reauthenticate it and refresh its
  recorded Google grant.
- Login displays a Google authorization URL, captures the Google loopback
  redirect automatically, exchanges the authorization code, stores the refresh token in the OS keyring,
  and saves account metadata, a key reference, and the exact granted scope set
  returned by Google in SQLite.
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
   The completion page attempts to close its browser tab; if the browser blocks
   script-initiated tab closing, it explains that the tab can be closed safely.
5. If the loopback listener cannot start, the TUI provides the legacy manual
   redirect-input fallback.

The authorization request uses Arqen's configured OpenID profile/email identity
and Gmail read-only policy. The TUI does not assume those permissions were
approved: it displays the exact scope set returned by Google's token exchange.
Accounts created before scope tracking show their scope state as unverified until
they are reauthenticated. Arqen records the last confirmed grant and does not
perform a live revocation check. The details view shows friendly labels with
canonical scope strings, including unknown provider scopes. The TUI itself is
not coupled to Hermes or any particular agent.

Connected, disconnected, and indeterminate connection states are persisted with
each identity. Disconnecting revokes the selected Google refresh token through
Google's OAuth revocation endpoint and removes the matching OS-keyring entry.
The identity and its last-confirmed scopes remain locally so the account can be
reconnected. If provider revocation or local cleanup cannot be completed, Arqen
shows an indeterminate state and offers retry or same-subject login recovery.

## Pane navigation and scrolling

The account list and selected-account details are independent, focusable panes.
Both panes reserve a persistent scrollbar track and thumb so a narrow terminal
still signals that more content is available; the thumb is visual only and is
not draggable in this version.

- `Tab` or `Shift+Tab` switches focus between the accounts and details panes.
- In the accounts pane, `j`/`k`, the arrow keys, `Home`, `End`, and
  `PageUp`/`PageDown` move the selected account. The selected row stays visible.
- In the details pane, the same keys scroll visual rows. Long and unknown scope
  strings, connection details, and additional information remain reachable.
- Mouse-wheel events scroll the pane under the pointer and focus it. Clicking an
  account row selects it; clicking the details badge keeps the existing
  disconnect/reconnect behavior.
- The footer exposes `[Tab] focus` and `[Wheel] scroll`. On wide layouts it
  also names the currently focused pane; compact layouts shorten labels to keep
  both panes visible. Wide footers separate shortcuts from status notices with
  a full-height vertical divider and wrap each column independently. Stacked
  narrow/compact footers keep shortcuts left-aligned and right-align the
  status notice.

Scroll offsets are runtime UI state. Selecting another account resets the
details pane to its top; no scroll position or scrollbar state is persisted.

## Security boundary

SQLite stores account metadata and a keyring reference. It does not store OAuth
access or refresh tokens. Refresh tokens are stored using the `keyring` crate,
which uses the persistent Linux Secret Service backend (with the keyutils cache)
in the flake development environment. Access tokens are held only during the
login exchange.

Older development builds used keyring's in-memory mock when no backend feature
was configured. Those tokens were never persisted and cannot be recovered;
affected account cards must be reauthenticated once after upgrading.

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
mount. Index coverage is best-effort: a `parse_partial` entry means the file was
indexed with the listed ranges potentially missing, while `not_indexed` entries
are deliberate exclusions. `Cargo.toml` is expected to report
`no_recorded_issue` after a fresh index; use direct source inspection for any
flagged path rather than treating graph absence as proof.

The graph UI setting is persisted in the ignored runtime state, not in the
committed MCP registration. In this checkout, `config list` currently reports
`ui_enabled=true` and `ui_port=9749`, so the server starts a loopback UI inside
the network-isolated container. `run.sh` does not publish a Docker port, so this
launcher does not make that UI reachable from the host. Check the local state
with:

```bash
bash .codex/mcp/codebase-memory/run.sh config list
```

Verify the built image, stdio protocol, tool list, representative queries, and
repository boundary with:

```bash
bash .codex/mcp/codebase-memory/test.sh
bash .codex/mcp/codebase-memory/run.sh cli list_projects
bash .codex/mcp/codebase-memory/run.sh cli index_status --project workspace-project
bash .codex/mcp/codebase-memory/run.sh cli check_index_coverage --project workspace-project --paths Cargo.toml
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

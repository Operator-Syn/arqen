# Arqen

An agent-agnostic Rust TUI for authenticating and managing multiple Google
accounts. The bundled MCP server can consume bounded Gmail metadata through a
host-side credential broker.

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
- Press `t` with an eligible account selected to set it as the single MCP
  target. Press `t` again to clear it; selecting another eligible account
  replaces the previous target. The target marker and status are persisted in
  SQLite.
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

For a repeatable local setup, run the named workflow once. It creates an
ignored `.env` from [`.env.example`](.env.example), creates a user-only bearer
token file, and loads those values automatically for every script:

```bash
make setup-local
make tui
```

Repository workflows use `.secrets/google-client-secret.json` by default. The
installed user services use `~/.config/arqen/google-client-secret.json`; set
`GOOGLE_CLIENT_SECRET` when a different path is required. The setup scripts
report a missing client without printing its contents; the TUI needs the
client only when a login starts.

The underlying command remains available when a script is not convenient:

```bash
cargo run
```

Then:

1. Press `a`.
2. Press `o` to open a dedicated Google login window, or press `c` to copy the
   authorization URL.
3. Choose the Google account and approve only the permissions you want to apply.
4. Arqen captures the loopback redirect, completes the login automatically, and
   terminates the dedicated browser process it started for this flow.
5. If the dedicated browser cannot be started, Arqen falls back to the normal
   browser launcher; close that browser window manually after login if needed.
6. If the loopback listener cannot start, the TUI provides the legacy manual
   redirect-input fallback.

The `LOGIN_HELPER_ENABLED` toggle in `src/main.rs` is disabled by default. Set it
to `true` to use the optional local user-gesture popup helper instead of the
TUI-owned browser process.

The owned browser uses a temporary, user-only profile rather than the user's
normal browser profile. It does not reuse existing browser sessions, and Arqen
removes the temporary profile when the login flow ends.

The authorization request uses Arqen's configured OpenID profile/email identity
and Gmail read-only policy. The TUI does not assume those permissions were
approved: it displays the exact scope set returned by Google's token exchange.
Accounts created before scope tracking show their scope state as unverified until
they are reauthenticated. Arqen records the last confirmed grant and does not
perform a live revocation check. The details view shows friendly labels with
canonical scope strings, including unknown provider scopes. The TUI itself is
not coupled to Hermes or any particular agent.

### Remote VPS login over SSH

When the TUI runs on a headless VPS, use remote OAuth mode and forward its
loopback callback to the laptop browser. The callback remains private to the
SSH connection; it is not exposed by the VPS firewall or reverse proxy:

```bash
ssh -t \
  -L 127.0.0.1:8765:127.0.0.1:8765 \
  user@your-vps \
  'ARQEN_OAUTH_REMOTE=1 ARQEN_OAUTH_CALLBACK_PORT=8765 ~/.local/bin/arqen'
```

Press `a` in the remote TUI, copy the displayed authorization URL from the SSH
terminal, and open it in the laptop browser. Keep the SSH connection open until
Arqen confirms the account. Change both port values together if `8765` is busy.

## Gmail MCP server

The first MCP capability is a bounded, read-only `list_emails` tool. It always
uses the account explicitly marked with `t` in the TUI; it never chooses an
account implicitly or accepts a subject from the remote caller. The tool lists
message IDs and fetches `From`, `Subject`, `Date`, labels, and a snippet (at
most 300 Unicode characters), with a public page-size cap of 50.

The server is split into two processes so refresh tokens stay on the host:

```text
TUI + SQLite + OS keyring ── Unix socket ── credential-broker
                                              │
                                      mcp-server (HTTP)
```

In the first-pass VPS deployment, the TUI and credential broker run on the VPS
host while `mcp-server` runs in the Docker container. The container receives a
read-only broker socket mount; it does not receive the SQLite database, OS
keyring, OAuth client JSON, or refresh tokens.

### Local workflow

The scripts keep the broker and HTTP process separate while avoiding repeated
exports. After `make setup-local`, start the host broker and MCP server in two
terminals:

```bash
make broker   # terminal A; same user as the TUI and keyring
make mcp      # terminal B; loopback HTTP on 127.0.0.1:8787
```

When the TUI is used only as the frontend for login and target selection, one
terminal can supervise both backend processes:

```bash
make backend
```

It initializes missing local configuration, waits for the broker socket, checks
the authenticated MCP health endpoint, and stops only the processes it started
when you press Ctrl-C. An already-running broker is reused. Add
`ARQEN_BACKEND_ARGS=--usurp` only to bypass the stale-socket confirmation.

If an interrupted broker left a socket behind, `make broker` checks whether a
listener is present and asks before taking over a stale socket. To explicitly
skip that prompt, use `make broker ARQEN_BROKER_ARGS=--usurp`. An active broker
is never overwritten.

For a disposable protocol check, `make smoke-local` builds the binary, starts
isolated broker/MCP processes, verifies authenticated `/healthz`, `/readyz`,
and `tools/list`, and cleans up its temporary socket, database, and token. It
does not call Google. `make smoke-local-call` additionally invokes
`list_emails` against the explicitly selected account, so use that only when a
live Gmail request is intended.

The ephemeral container path is similarly named:

```bash
make compose-smoke       # build/run a temporary Compose MCP process
make compose-smoke-call  # same, plus one live list_emails call
make compose-up          # persistent container; broker must already run
make compose-down
```

All scripts source the ignored `.env`. Its example values keep development on
loopback, use `.secrets/mcp-bearer-token`, and select the repository's pinned
Nix shell when available (`ARQEN_USE_NIX=auto`). The Compose scripts pass the
current UID/GID so the container can read only the host broker socket; they do
not start the broker for you.

The direct commands remain useful for manual or non-local deployments:

```bash
cargo run -- credential-broker
cargo run -- mcp-server
```

For those direct commands, set `ARQEN_MCP_ALLOWED_HOSTS`,
`ARQEN_MCP_ALLOWED_ORIGINS`, and either `ARQEN_MCP_BEARER_TOKEN_FILE` or
`ARQEN_MCP_BEARER_TOKEN` outside Git. Keep the listener behind a TLS reverse
proxy when it is not loopback.

`ARQEN_MCP_LISTEN_ADDR` defaults to `127.0.0.1:8787` and
`ARQEN_GMAIL_BROKER_SOCKET` defaults to
`${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`. The bearer token can instead be
provided as `ARQEN_MCP_BEARER_TOKEN`; keep either value outside Git and
user-readable only. Put a TLS reverse proxy in front of the HTTP listener for
public access. See [the architecture docs](docs/README.md),
[`Dockerfile`](Dockerfile),
[`deploy/containers/docker-compose.yml`](deploy/containers/docker-compose.yml),
and [`deploy/nginx/arqen-mcp.conf`](deploy/nginx/arqen-mcp.conf) for the
operator-owned examples.

The Gmail read-only OAuth scope is restricted. Review Google’s current
verification and user-data policy before any public deployment. The checked-in
server, container, systemd, and Nginx files do not create secrets, issue
certificates, change DNS/firewall state, activate services, or deploy a live
endpoint.

### Unattended user services

Build and prepare the VPS host-side broker plus Docker MCP workflow with:

```bash
make vps-up
```

This builds the host binary, prepares XDG config paths and a user-only bearer
token file, enables the host credential broker with user lingering, and starts
the Docker MCP container. The native MCP unit is retained as an alternative
but is not enabled by the first-pass VPS workflow.

To prepare without activation:

```bash
make quickstart
```

The MCP service exposes authenticated `/healthz` for process liveness and
`/readyz` for broker/database/target/keyring readiness. A missing target keeps
the service alive but returns a not-ready result and `target_not_configured`
for tool calls. Inspect service state with `systemctl --user` and
`journalctl --user`; no service activation or public deployment is performed
by repository checks.

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
in the flake development environment. Access tokens are held only in memory
during login and short-lived broker requests; they are never persisted or
returned to the MCP server.

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

The complete repository check sequence is also available as:

```bash
make check
```

The scripts are small Bash wrappers around the same Cargo and Nix commands;
they are not a second build system and do not hide live OAuth/Gmail work.

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

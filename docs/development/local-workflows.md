# Local workflows

The repository includes a small set of named Bash workflows so a development
session does not require repeatedly exporting the same values. The tracked
`.env.example` contains loopback defaults and paths only; copy it to the
ignored `.env` with:

```bash
make setup-local
```

That command creates `.env` with mode `0600`, creates `.secrets/` with mode
`0700`, and generates `.secrets/mcp-bearer-token` with mode `0600` when a token
file is configured and missing. It never generates or prints Google OAuth
credentials. Add the real desktop-client JSON at the configured
`GOOGLE_CLIENT_SECRET` path before starting a login or making a live Gmail
request.

## Named commands

| Command | Purpose | Live Google request? |
| --- | --- | --- |
| `make quickstart` | build/install the binary and prepare native user units without activation | no |
| `make vps-up` | enable the host broker and start the Docker MCP service | no, until a tool call arrives |
| `make tui` | start the interactive account TUI | only when the user starts login |
| `make backend` | set up and supervise the native broker + MCP backend | no, until a tool call arrives |
| `make broker` | start the host keyring/SQLite credential broker; asks before stale-socket takeover | no, until an MCP call arrives |
| `make mcp` | start the authenticated Streamable HTTP MCP process | no, until a tool call arrives |
| `make check` | format, test, lint, build, flake, and diff checks | no |
| `make smoke-local` | disposable native broker + MCP protocol smoke | no |
| `make smoke-local-call` | native smoke plus one `list_emails` call | yes |
| `make compose-up` / `make compose-down` | manage the configured local container | no, until a tool call arrives |
| `make compose-smoke` | disposable Compose protocol smoke | no |
| `make compose-smoke-call` | Compose smoke plus one `list_emails` call | yes |

Each target delegates to a correspondingly named executable in `scripts/`.
`scripts/lib/common.sh` loads `.env`, resolves repository-relative paths, and
uses `nix develop .#arqen -c cargo ...` when `ARQEN_USE_NIX=auto` and Nix is
available. Set `ARQEN_USE_NIX=never` in `.env` to use the system Cargo
toolchain, or `always` to require Nix.

## One-command backend

When the TUI is treated as the frontend, use the backend supervisor:

```bash
make backend
```

It runs local setup if `.env` has not been created, validates the OAuth client
and bearer-token file, starts the host broker, waits for its Unix socket, starts
the native MCP HTTP server, checks authenticated `/healthz`, and keeps both
processes attached to one terminal. Press Ctrl-C to stop the processes started
by that command. If a broker was already running, it is reused and is not
stopped by the supervisor.

Use `make backend ARQEN_BACKEND_ARGS=--usurp` to skip the stale-socket prompt.
This only takes over a stale Unix socket; active or unknown resources remain
protected. The container workflows remain explicit (`make compose-up` or
`make compose-smoke`) so a backend command cannot unexpectedly remove a
persistent container.

## Native two-process run

Start these under the same user as the TUI so the broker can access the OS
keyring:

```bash
make broker   # terminal A
make mcp      # terminal B
```

If an interrupted broker left a socket with no listener, `make broker` checks
it before starting and asks whether to take it over. Use the explicit flag only
from a trusted terminal when you want to skip that prompt:

```bash
make broker ARQEN_BROKER_ARGS=--usurp
```

`--usurp` can remove only a stale Unix socket; an active broker is never
overwritten. The broker wrapper also removes its own stale socket on normal
script exit. A socket that cannot be classified safely is left untouched.
The Compose startup wrapper uses the same classification and refuses to start
against a stale or unknown broker path; it never kills a process to reclaim a
TCP port or Compose project.

The example listener is `127.0.0.1:8787`. The MCP process reads the bearer
token from `.secrets/mcp-bearer-token`; the broker uses the default
`${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`. The TUI's `t` action must have
already selected one eligible connected account before `list_emails` can
succeed.

## Remote OAuth over SSH

For a TUI running on a headless VPS, use a fixed loopback callback and an SSH
local forward from the laptop:

```bash
ssh -t \
  -L 127.0.0.1:8765:127.0.0.1:8765 \
  user@your-vps \
  'ARQEN_OAUTH_REMOTE=1 ARQEN_OAUTH_CALLBACK_PORT=8765 ~/.local/bin/arqen'
```

Press `a` in the remote TUI, open the displayed URL in the laptop browser, and
keep the SSH connection open until the callback completes. The callback port
is loopback-only on both ends and must not be opened in a firewall.

## Unattended user services

`make quickstart` installs the release binary under `~/.local/bin`, creates
`~/.config/arqen/arqen.env` and the user-only bearer token file, installs
`arqen-credential-broker.service` and `arqen-mcp.service`, and reloads the
user systemd manager. It does not enable or start them unless explicitly
requested:

```bash
make quickstart ARQEN_QUICKSTART_ARGS='--enable --enable-linger'
make compose-up
```

For the locked first-pass VPS deployment, use `make vps-up`; it enables only
the host broker and starts the Docker MCP service. The native MCP unit is an
alternative for hosts that do not use Docker. The Docker MCP process remains
live while the broker recovers. Authenticated
`/healthz` reports HTTP liveness; `/readyz` reports local broker/database/
target/keyring readiness without calling Gmail. A service restart requires MCP
clients to initialize again, but account and target configuration remain in
SQLite.

## Disposable smoke checks

The native smoke script builds the binary, creates a temporary runtime/socket,
isolated database, and bearer token, starts both processes, verifies
authenticated `/healthz`, `/readyz`, and `tools/list`, then terminates both
processes and removes only its generated temporary directory. The no-call path
expects readiness to report `target_not_configured` and cannot touch Google.
The Compose variant additionally builds and runs the container with the host
broker socket mounted read-only and cleans up its exact Compose project.

Pass `--call` only when you intentionally want to exercise Gmail. That path
uses the configured real OAuth client, keyring, database, and explicitly
selected target account; failures are reported without printing the response
or any credential.

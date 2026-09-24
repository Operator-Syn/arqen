# Local-first workflows

The repository supports a Docker-native local profile and a native loopback
workflow. Docker runs the clean-slate stack; native mode uses the host OS
keyring. Named commands load settings from the ignored `.env` file. The tracked
`.env.example` contains local defaults and paths, not credentials. Create `.env`
with:

```bash
make setup-local
```

Setup copies `.env.example` only when `.env` does not exist; it does not merge
or overwrite an existing file. If the example changes later, update your local
`.env` deliberately while preserving any local choices. The command creates
`.env` with mode `0600`, creates `.secrets/` with mode `0700`, and generates
`.secrets/mcp-bearer-token` with mode `0600` when a token file is configured and
missing. It never generates or prints Google OAuth credentials. Add the real
desktop-client JSON at the configured
`GOOGLE_CLIENT_SECRET` path before starting a login or making a live Gmail
request. Then run `make tui`, complete the Google login, and press `t` on the
eligible account that should be exposed to local agents.

## Named commands

| Command | Purpose | Live Google request? |
| --- | --- | --- |
| `make docker-setup` | initialize the clean-slate Docker-native OpenBao profile and protected local setup files | no |
| `make docker-up` | start the Docker-native control gateway/TUI, broker, and MCP stack and wait for control/MCP liveness | no |
| `make docker-status` | report container, control gateway, OpenBao, MCP liveness, and MCP readiness without printing secrets | no |
| `make docker-down` | stop the Docker-native stack while preserving its volumes | no |
| `make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES` | explicitly delete the fresh Docker-native volumes and generated setup secrets | no |
| `make quickstart` | build/install the binary and prepare native user units without activation | no |
| `make tui` | start the interactive account TUI | only when the user starts login |
| `make backend` | set up and supervise the native broker + MCP backend | no, until a tool call arrives |
| `make broker` | start the host keyring/SQLite credential broker; asks before stale-socket takeover | no, until an MCP call arrives |
| `make mcp` | start the authenticated Streamable HTTP MCP process | no, until a tool call arrives |
| `make check` | format, test, lint, build, flake, and diff checks | no |
| `make smoke-local` | disposable native broker + MCP protocol smoke | no |
| `make smoke-local-call` | native smoke plus one `list_emails` call | yes |

Each target delegates to a correspondingly named executable in `scripts/`.
`scripts/lib/common.sh` loads `.env`, resolves repository-relative paths, and
uses `nix develop .#arqen -c cargo ...` when `ARQEN_USE_NIX=auto` and Nix is
available. Set `ARQEN_USE_NIX=never` in `.env` to use the system Cargo
toolchain, or `always` to require Nix.

## Repository code-knowledge MCP

This is separate from Arqen's Gmail `/mcp` service. The project-scoped
`codebase-memory-mcp` gives coding agents graph-backed symbol search,
call-path, architecture, and impact queries for this checkout. It is registered
in [`.mcp.json`](../../.mcp.json) and [`.codex/config.toml`](../../.codex/config.toml);
it does not change an agent's global MCP configuration.

The one-time setup requires Docker and network access to build the pinned image,
then creates the ignored local graph state and indexes the repository:

```bash
bash .codex/mcp/codebase-memory/setup.sh
```

After setup, restart or reconnect the MCP client. The agent is ready when its
tool list includes `mcp__codebase_memory_mcp__...` (or the equivalent client
namespaced tools). Validate the local image, stdio handshake, tool surface,
repository boundary, and current index with:

```bash
bash .codex/mcp/codebase-memory/test.sh
bash .codex/mcp/codebase-memory/run.sh cli list_projects
bash .codex/mcp/codebase-memory/run.sh cli index_status --project workspace-project
bash .codex/mcp/codebase-memory/run.sh cli check_index_coverage \
  --project workspace-project --paths Cargo.toml
```

Use this prompt when opening a fresh coding-agent session in this repository:

```text
You are working in this Git checkout. Set up and use only the repository-scoped
codebase-memory-mcp. Read AGENTS.md and the repository MCP instructions first.
If Docker or the pinned image is missing, run
`bash .codex/mcp/codebase-memory/setup.sh`; do not install a host-global runtime
or edit global MCP configuration. Run
`bash .codex/mcp/codebase-memory/test.sh`, then verify `list_projects`,
`index_status --project workspace-project`, and
`check_index_coverage --project workspace-project --paths Cargo.toml`.
If the MCP tools are not listed after setup, stop and ask me to restart or
reconnect this agent session. Once loaded, call `list_projects` and
`index_status` before non-trivial discovery. Use direct source inspection when
coverage is partial or stale, and do not expose secrets or run graph-mutating
tools without the required approval.
```

The graph is best-effort evidence, not a replacement for source inspection or
tests. Setup and indexing do not prove that the separate Gmail MCP endpoint,
OAuth flow, or any live service is available.

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

Native local agents connect to the shared endpoint at
`http://127.0.0.1:8787/mcp` with an `Authorization: Bearer` header whose value
comes from `.secrets/mcp-bearer-token`. The default Host allowlist is exactly
`127.0.0.1:8787`; a supplied Origin must be
`http://127.0.0.1:8787`, while an omitted Origin is accepted by the MCP
transport. If a client uses `localhost:8787`, add matching host and origin
values to `.env` explicitly.

The native broker and TUI must run under the same user so the broker can use the
same SQLite database, runtime socket, and OS keyring session. Every local agent
that can read the bearer token shares the same single-operator access. The
server exposes read-only `list_emails` and `read_email` tools and always uses
the one account selected with `t` in the TUI; remote callers cannot choose
another account. Use a `list_emails` result's `id` as `read_email.message_id`.
Email text is untrusted content, and message/response sizes are bounded.

Use authenticated `GET /healthz` for HTTP liveness and `GET /readyz` for local
broker/database/target/protected-credential readiness. `/readyz` does not call Gmail. A
successful `list_emails` call is the first live provider check.

Use `make backend ARQEN_BACKEND_ARGS=--usurp` to skip the stale-socket prompt.
This only takes over a stale Unix socket; active or unknown resources remain
protected. The backend supervisor manages only the host processes it starts.

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
already selected one eligible connected account before `list_emails` or
`read_email` can succeed.

## Remote OAuth over SSH

SSH forwarding remains supported for logging in to a TUI on a headless host.
It forwards only the OAuth callback; the retired host-broker-plus-Docker-MCP
Compose deployment is not part of this workflow.

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

## Unattended local user services

`make quickstart` installs the release binary under `~/.local/bin`, creates
`~/.config/arqen/arqen.env` and the user-only bearer token file, installs
`arqen-credential-broker.service` and `arqen-mcp.service`, and reloads the
user systemd manager. It does not enable or start them unless explicitly
requested:

```bash
make quickstart ARQEN_QUICKSTART_ARGS='--enable --enable-native-mcp --enable-linger'
```

This builds and installs the release binary, creates the user-only config and
token paths, enables both native services, and enables user lingering so the
loopback MCP endpoint can remain available after logout. It does not create
OAuth credentials or select an account; complete those steps in the TUI. A
foreground `make backend` session remains the simpler choice when persistence
is not needed. Authenticated `/healthz` reports HTTP liveness; `/readyz` reports
local broker/database/target/keyring readiness without calling Gmail. A service
restart requires MCP clients to initialize again, but account and target
configuration remain in SQLite.

## Docker-native local stack

For a clean-slate deployment with no native Arqen processes, initialize and
start the complete Compose stack:

```bash
make docker-setup   # first run only; creates protected local setup secrets
make docker-up
```

By default, `docker-up` builds the app images from the current checkout. To use
GHCR instead, set `ARQEN_DOCKER_IMAGE_SOURCE=registry` and optionally
`ARQEN_DOCKER_IMAGE_TAG=<version>` in `.env`; startup pulls the control/broker
runtime and MCP images, then starts them with `--no-build`. For an offline
AMD64 transfer bundle, run `make docker-bundle`, then set
`ARQEN_DOCKER_IMAGE_SOURCE=bundle`. The bundle at
`out/arqen-docker-stack/` contains two OCI layouts, a Docker-loadable archive,
and `services.json`. It does not contain `.secrets` or other runtime
credentials. The registry and bundle modes keep the same mounted-secret and
service boundaries as source-build mode.

Pushes to `main` trigger publication of versioned and `latest` multi-platform
images to GHCR. Once both images publish, the workflow tags the source and
increments only the patch version in `Cargo.toml` and `Cargo.lock`; set
major/minor versions manually in `Cargo.toml`, then run `cargo check` so the
root package version in `Cargo.lock` matches. The `docker-images` branch is
metadata-only and records per-release image digests, pull commands, and a
latest index. The first GHCR publication requires an operator to change both
package visibilities to public. Details and setup requirements are in
[`../operations/docker-images.md`](../operations/docker-images.md).

The stack runs OpenBao, the credential broker, the control gateway/TUI, and the
MCP server in Docker. OpenBao is internal-only; only loopback ports for the
control gateway (`7681`), OAuth callback (`8765`), and MCP (`8787`) are
published. ttyd stays on the control container's loopback port `7682`.
OpenBao refresh-token references use separate control and broker AppRoles. Open
`http://127.0.0.1:7681`, enter username `arqen` and the generated control
password from `.secrets/arqen-control-password`, complete OAuth in the TUI, and
press `t` to set the target. The gateway has no `WWW-Authenticate` challenge;
failed credentials stay on the themed page and successful login receives a
memory-only 12-hour session cookie.

Each `make docker-up` run checks the control and broker AppRole credentials
independently before refreshing the containers' secret caches. If OpenBao
rejects one role's credentials, the one-shot bootstrap replaces that role's
RoleID and SecretID, reapplies its policy and lifetime settings, and verifies
the replacement. It leaves a working role unchanged and preserves OpenBao KV
data. A network or OpenBao availability failure stops startup without rotating
either role. OAuth error dialogs identify the failed stage and state whether
the protected refresh token and account-scope metadata were saved.

The MCP container receives only the bearer-token file and broker socket. OAuth
client JSON and all OpenBao role credentials stay in the control or broker
containers.

Run `make docker-up` once from the graphical host session that owns the
clipboard. The startup script prefers Wayland and falls back to X11, validates
the selected socket, and adds only that display interface to `arqen-control`.
The headless OpenBao, broker, and MCP services may be restored by the system
Docker daemon, but `arqen-control` is deliberately not a boot-restarted
container: its native display socket is session-owned. `make quickstart`
installs `arqen-docker-control.service`; enable that user unit when the control
container should start automatically after the graphical session is ready. The
session startup runs the one-shot OpenBao unseal helper before starting the
control container, so a headless OpenBao restored sealed by Docker is available
again without moving its unseal key into a long-running container.
The local Arqen app containers run as the invoking non-root host UID/GID so
the shared SQLite, broker socket, and compositor authorization stay coherent.
Press `c` on the authorization screen to use Arqen's native clipboard path;
the exact URL is copied without inspecting ttyd output. Broker, MCP, and
OpenBao remain display-isolated. A headless Docker host should use the native
TUI/manual OAuth workflow instead.

The Docker control terminal pins ttyd's DOM renderer because the pinned ttyd
1.7.7 WebGL preference can leave stale cell measurements on first load. The
DOM renderer makes the streamed terminal fill the browser viewport immediately;
the small inset inside the Arqen TUI is part of its intentional layout.

Inspect or stop the stack without deleting its persistent volumes:

```bash
make docker-status
make docker-down
```

The explicit reset path removes the fresh Docker profile and generated
OpenBao/control secrets:

```bash
make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES
```

This profile intentionally starts with a new SQLite/OpenBao data volume; it
does not migrate older native keyring accounts.

## Disposable smoke checks

The native smoke script builds the binary, creates a temporary runtime/socket,
isolated database, and bearer token, starts both processes, verifies
authenticated `/healthz`, `/readyz`, and `tools/list`, then terminates both
processes and removes only its generated temporary directory. The no-call path
expects readiness to report `target_not_configured` and cannot touch Google.

Pass `--call` only when you intentionally want to exercise Gmail. That path
uses the configured real OAuth client, keyring, database, and explicitly
selected target account; failures are reported without printing the response
or any credential.

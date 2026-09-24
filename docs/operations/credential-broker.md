# Running the credential broker

The native broker is deliberately host-side so the process that can access the
OS keyring and SQLite database is not the public HTTP process. The Docker-native
profile uses the same Unix-socket boundary but runs the broker in its own
container and resolves refresh tokens through OpenBao instead of the host
keyring; see [the Docker-native local workflow](../development/local-workflows.md#docker-native-local-stack).

```bash
cargo run -- credential-broker
```

For the repository's local workflow, `make broker` loads `.env`, selects the
same-user runtime directory, checks for an existing socket, and starts this
command. Run `make setup-local` first so the OAuth client path is explicit; the
broker still needs the real client JSON before it can perform a live Gmail
request. If an interrupted run left a socket with no listener, the wrapper
asks before taking it over. Use `make broker ARQEN_BROKER_ARGS=--usurp` to
explicitly skip that prompt.

The binary resolves the database and OAuth client paths using the same XDG and
`GOOGLE_CLIENT_SECRET` rules as the TUI. Set `XDG_RUNTIME_DIR`; the default
socket is `${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`. An active broker is
never replaced. A stale socket is removed only when it is a Unix socket with no
listener. The included
`deploy/systemd/arqen-credential-broker.service` is a user-unit template; the
paired `arqen-mcp.service` is an all-native alternative to the Docker-native
local profile.

Set `ARQEN_GMAIL_BROKER_SOCKET` in `.env` only when an explicit socket path is
needed. Otherwise the scripts and binary use
`${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`; the broker creates the parent
directory with user-only permissions and refuses to replace an existing
socket.

The broker must run under the same user (or an explicitly coordinated UID/GID)
as the TUI keyring session. For unattended use, enable systemd user lingering
and ensure that the user’s headless Secret Service remains available after
logout. If the broker is unavailable, MCP readiness is `503` and tool calls
return an internal broker error; it does not fall back to reading credentials
from the MCP process or container.

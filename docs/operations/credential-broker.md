# Running the credential broker

The broker is deliberately host-side so the process that can access the OS
keyring and SQLite database is not the public HTTP process.

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
socket is `${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`. A second broker does
not replace an existing socket path; stop the previous broker and remove a
stale socket deliberately before restarting. The included
`deploy/systemd/arqen-credential-broker.service` is a user-unit template; copy
and adapt its binary/client-secret paths before enabling it.

Set `ARQEN_GMAIL_BROKER_SOCKET` in `.env` only when an explicit socket path is
needed. Otherwise the scripts and binary use
`${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock`; the broker creates the parent
directory with user-only permissions and refuses to replace an existing
socket.

The broker must run under the same user (or an explicitly coordinated UID/GID)
as the TUI keyring session. It is expected to be running before the MCP
container starts. If it is unavailable, MCP returns an internal broker error;
it does not fall back to reading credentials from the container.

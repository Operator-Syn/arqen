# Runtime topology and lifecycle

**Status:** `verified-repository` for source and checked-in templates;
`assumption` for host/container wiring until an operator deploys it.

The intended deployment separates the process that can read credentials from
the process that is reachable over the network:

```mermaid
sequenceDiagram
    participant T as Arqen TUI
    participant S as SQLite
    participant K as OS keyring
    participant B as credential-broker (host)
    participant M as mcp-server (container)
    participant N as Nginx (public TLS)
    participant G as Gmail API
    T->>S: mark one connected subject as MCP target
    T->>K: store refresh token during OAuth login
    M->>N: public HTTPS request arrives
    N->>M: HTTP/1.1 /mcp with Host/Origin
    M->>B: bounded JSON request over Unix socket
    B->>S: read target and eligibility
    B->>K: read/refresh token (never return it)
    B->>G: list IDs and get metadata
    G-->>B: bounded summaries
    B-->>M: result or stable error code
    M-->>N: MCP JSON/SSE response
```

The broker socket defaults to `${XDG_RUNTIME_DIR}/arqen/gmail-broker.sock` and
is created with a user-only directory/socket mode. The container example
mounts that socket read-only and binds its HTTP port to loopback; Nginx is the
public TLS boundary. The service templates do not create certificates, DNS,
firewall rules, users, or secret files.

Credential failures are fail-closed. A stale or disconnected target is not
replaced automatically; a missing target returns `target_not_configured`, and
an expired/revoked Google grant returns `reauthentication_required` without
changing SQLite connection state.

MCP transport sessions are process-local memory in the HTTP server. They are
not durable account configuration and are not written to SQLite; a server
restart requires a client handshake again.

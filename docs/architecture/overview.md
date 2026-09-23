# Architecture overview

**Status:** `verified-repository` for the components and paths below.

Arqen has one Rust package and one binary with four runtime entry points:
the default TUI, `control-gateway`, `credential-broker`, and `mcp-server`.
The Docker-native local profile runs the gateway/TUI, broker, and MCP roles in
one Compose project, with OpenBao as the refresh-token store and ttyd kept
behind the gateway. Native user services and the host-broker/VPS Compose path
remain compatibility alternatives. The MCP server never reads a credential
store directly. It asks the broker to resolve the one explicitly selected
Google subject and to call Gmail with a short-lived access token.

```mermaid
flowchart LR
    client[Remote MCP client] -->|HTTPS via reverse proxy| edge[Nginx or equivalent]
    edge -->|HTTP /mcp| server[arqen mcp-server]
    server -->|JSON line over Unix socket| broker[arqen credential-broker]
    broker --> db[(SQLite metadata)]
    broker --> store[(OpenBao KV or native keyring)]
    broker -->|HTTPS Gmail API| gmail[Google Gmail]
    browser[Local browser] --> gateway[control-gateway :7681]
    gateway --> ttyd[ttyd :7682 loopback]
    ttyd --> tui[Arqen TUI]
    tui --> db
    tui --> store
    tui -->|target subject configuration| db
```

Responsibilities are intentionally narrow:

| Boundary | Owns | Does not own |
| --- | --- | --- |
| Control gateway (`src/control/`) | Local password page, memory-only browser sessions, ttyd HTTP/WebSocket proxy | OAuth, account choice, password persistence, public exposure |
| TUI (`src/tui/`, `src/ui/`) | Account login/logout, loopback or container-published OAuth callback, target selection, presentation | MCP HTTP, service lifecycle, access-token caching |
| Store (`src/store/`, `src/lib.rs`) | Account metadata, exact granted scopes, singleton target subject | Refresh/access token values |
| OAuth (`src/auth/`) | PKCE, callback exchange, refresh, protected-store coordinates | Gmail message presentation |
| Broker (`src/broker/`) | Target validation, refresh-token read, access-token cache, Gmail call | Public network listener, MCP sessions |
| Gmail client (`src/gmail/`) | Bounded list/get metadata requests and summaries | Token persistence |
| MCP server (`src/server/`, `src/mcp.rs`) | Streamable HTTP, bearer gate, tool schema, wire errors | Credential-store access and account choice |
| systemd user units (`deploy/systemd/`) | Host broker restart and optional all-native MCP lifecycle | OAuth consent, secret creation, public deployment |
| Docker-native Compose (`deploy/containers/docker-native-compose.yml`) | OpenBao, control TUI, broker, and loopback MCP lifecycle; native display access is limited to the control override | Public exposure, live OAuth consent, account migration |
| Legacy Docker Compose (`deploy/containers/docker-compose.yml`) | First-pass always-on MCP HTTP boundary | SQLite, native keyring, OAuth, account choice |

The MCP exposes read-only `list_emails` and `read_email` tools for the
configured target. `list_emails` returns bounded metadata/snippets only; an
agent can pass one result's `id` to `read_email` to retrieve decoded body text.
Neither tool accepts an account-selection parameter, and attachments are not
downloaded.

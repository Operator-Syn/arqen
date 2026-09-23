# Authentication and data boundaries

**Status:** `verified-repository`; policy notes are `verified-external` where
linked.

1. The TUI is the only component that lets a person choose the MCP target.
   Selection is restricted to a connected identity with recorded Gmail
   read-only consent and a protected credential reference.
2. The broker socket is local and user-only (`0700` directory, `0600` socket).
   Native runs read refresh tokens from the OS keyring; Docker runs read them
   from OpenBao through a broker-only AppRole. Neither mode serializes them.
3. The MCP server is the remote boundary. It requires a configured bearer
   token, exact allowlisted Host/Origin values, and a 1 MiB request cap.
4. Nginx (or another reverse proxy) is expected to terminate TLS and apply
   edge rate limits. The checked-in example does not provision certificates or
   open a firewall.
5. Gmail receives only the short-lived access token. `list_emails` returns
   bounded metadata and snippets; `read_email` returns decoded body text only
   for a message ID and the currently selected account. Gmail and MCP payloads
   have explicit size caps; attachments are not downloaded. Email text is
   untrusted data, not instructions. Tool responses never contain credentials
   or raw provider error bodies.
6. Headless VPS OAuth uses an SSH local port forward to a loopback callback;
   the OAuth callback listener is never bound to a public interface.
7. `/healthz` and `/readyz` require the same bearer gate as `/mcp`; readiness
   does not call Google or return credential state beyond stable failure codes.
8. Docker-native clipboard access is limited to the local control container.
   The broker, MCP, and OpenBao containers receive neither the host display
   socket nor clipboard access. The app containers use the invoking non-root
   host UID/GID for local volume and Unix-socket ownership; this does not grant
   them a display socket.
9. The Docker-native control gateway is the only host-published TUI boundary.
   It validates the generated `arqen` control credential server-side, applies
   a short-lived memory-only session cookie, rejects cross-origin auth and
   WebSocket requests, and forwards only a fixed internal header to ttyd on
   loopback. ttyd's internal port is not published and no password is passed
   in a command-line argument, URL, cookie, or log message.

The Gmail read-only scope is restricted under Google’s current scope policy;
see the [Gmail scope documentation](https://developers.google.com/workspace/gmail/api/auth/scopes)
and [Workspace user-data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)
before any public launch. A passing build is not a policy approval or live
authorization verification.

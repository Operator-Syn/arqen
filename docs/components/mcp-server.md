# Streamable HTTP MCP server

**Source:** `src/server.rs`, protocol wire types: `src/mcp.rs`.

`arqen mcp-server` exposes one endpoint at `/mcp` using the official Rust MCP
SDK’s Streamable HTTP server. The current deployment mode uses JSON responses
and retains the SDK’s request-scoped streaming capability for responses that
need SSE. `/healthz` is an authenticated no-content probe.

The SDK’s local session manager is process-local; MCP session state is not
stored in the Arqen SQLite database. Restarting the HTTP process therefore
requires clients to initialize again, while the selected Google target remains
persisted independently in SQLite.

The v1 edge gate requires an exact `Authorization: Bearer <token>` value,
configured through `ARQEN_MCP_BEARER_TOKEN` or a user-only token file. The
SDK additionally validates configured `Host` and `Origin` allowlists and the
HTTP body is capped at 1 MiB. This bearer gate is intentionally a private
single-operator control for v1; MCP-native OAuth authorization is a later
decision, not implied by the current route.

The server advertises only `list_emails`. Tool failures are returned as
stable broker-code-prefixed messages while the broker keeps provider and
credential details private.

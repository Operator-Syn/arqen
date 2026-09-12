# Authentication and data boundaries

**Status:** `verified-repository`; policy notes are `verified-external` where
linked.

1. The TUI is the only component that lets a person choose the MCP target.
   Selection is restricted to a connected identity with recorded Gmail
   read-only consent and a keyring reference.
2. The broker socket is local and user-only (`0700` directory, `0600` socket).
   It reads refresh tokens from the OS keyring and never serializes them.
3. The MCP server is the remote boundary. It requires a configured bearer
   token, exact allowlisted Host/Origin values, and a 1 MiB request cap.
4. Nginx (or another reverse proxy) is expected to terminate TLS and apply
   edge rate limits. The checked-in example does not provision certificates or
   open a firewall.
5. Gmail receives only the short-lived access token and bounded metadata
   requests. Public tool responses contain no full bodies, attachments, or
   credentials.

The Gmail read-only scope is restricted under Google’s current scope policy;
see the [Gmail scope documentation](https://developers.google.com/workspace/gmail/api/auth/scopes)
and [Workspace user-data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)
before any public launch. A passing build is not a policy approval or live
authorization verification.


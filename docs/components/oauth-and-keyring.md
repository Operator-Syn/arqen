# OAuth and keyring boundary

**Source:** `src/auth.rs`; metadata consumer: `src/lib.rs`.

Google login uses authorization code + PKCE. The token exchange requires a
non-empty `scope` response; Arqen canonicalizes that exact set and stores it
in `google_account_scopes`. Refresh tokens remain in the OS keyring under the
stored `keyring:<service>:<user>` reference. SQLite stores only account
metadata, the reference, connection state, and scope evidence.

The broker calls `refresh_google_access_token` with the selected subject,
rotates a returned refresh token in the same keyring entry, and keeps the
access token in an in-memory, expiry-aware cache. Neither the broker protocol
nor MCP result/error payload contains a token. Google `invalid_grant` and
Gmail HTTP 401 are surfaced as a reauthentication error; the broker does not
silently rewrite the account state.


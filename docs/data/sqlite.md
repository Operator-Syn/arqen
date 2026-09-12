# SQLite data model

**Source:** `src/lib.rs`.

The existing `google_accounts` table remains the identity source of truth. A
row is keyed by Arqen’s local `id` and has a unique Google `google_subject`,
email/display name, a keyring reference, and a persisted connection state.
Exact granted scopes are normalized into `google_account_scopes` and cascade
when an identity is deleted.

The migration adds a singleton `mcp_configuration` row (`id = 1`) with
`target_google_subject`. The foreign key uses `ON DELETE SET NULL`, so deleting
the selected identity clears configuration while disconnecting it leaves the
stale subject visible for fail-closed diagnostics. `set_mcp_target_subject`
accepts only a connected account with the exact Gmail read-only scope and a
non-null keyring reference; replacing a target is one SQLite transaction.

No access token, refresh token, OAuth client secret, or keyring password is
stored in SQLite. The target subject is configuration, not a second credential
or an authorization grant. MCP HTTP session state is in-memory in the server
process and is intentionally not part of this database.

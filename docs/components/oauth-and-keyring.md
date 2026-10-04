# OAuth and protected credential-store boundary

**Source:** `src/auth/`; metadata consumer: `src/store/` and `src/lib.rs`.

Google login uses authorization code + PKCE. The token exchange requires a
non-empty `scope` response; Arqen canonicalizes that exact set and stores it
in `google_account_scopes`. Docker refresh tokens remain in OpenBao KV v2 under
an `openbao:arqen/google/<subject>` reference; native runs use the OS keyring
under the stored `keyring:<service>:<user>` reference. SQLite stores only
account metadata, the opaque reference, connection state, and scope evidence.

Arqen requests both Gmail's restricted `gmail.readonly` and `gmail.modify`
scopes. The latter permits reading, composing, and sending email; the product
uses its write access for label management/application, per-message read-state,
the guarded message-to-Trash flow, and guarded Gmail draft create/delete/send
workflows. Sending requires a separate one-use action marker and explicit user
authorization for the exact draft.
The persisted target-selection requirement remains `gmail.readonly`, while the
broker checks for an exact recorded `gmail.modify` grant before label create,
apply, delete, read-state, message-to-Trash, or draft create/delete/send
operations. Reauthorize the
selected account
after deploying code that requests this new grant: an existing refresh token
does not inherit newly requested scopes. Google also limits refresh tokens for
external OAuth apps in Testing to seven days when Gmail scopes are requested.

The broker calls `refresh_google_access_token` with the selected subject,
rotates a returned refresh token in the same protected-store entry, and keeps the
access token in an in-memory, expiry-aware cache. Neither the broker protocol
nor MCP result/error payload contains a token. Google `invalid_grant` and
Gmail HTTP 401 are surfaced as a reauthentication error; the broker does not
silently rewrite the account state.

OAuth completion stores the refresh token in the protected backend before
writing account and granted-scope metadata to SQLite. The TUI reports Google
authorization, protected-store, and SQLite failures separately without showing
provider response bodies or transport details. If OpenBao rejects an AppRole
login, `make docker-up` validates and repairs the affected local service role
before the TUI is retried. Google consent may already have completed even when
Arqen could not persist the refresh token or local scope record.

On a headless VPS, set `ARQEN_OAUTH_REMOTE=1` and a fixed
`ARQEN_OAUTH_CALLBACK_PORT` (default `8765`), then forward that loopback port
from the laptop with SSH. The browser remains local while the code exchange
and protected-store write happen on the VPS. Installed services use the XDG client
configuration path; `GOOGLE_CLIENT_SECRET` remains an explicit override.

The loopback listener (`src/callback/`) accumulates HTTP headers until the
complete `\r\n\r\n` terminator, within an 8192-byte cap and the existing
two-second socket-read timeout. TCP reads need not contain a complete request.
Incomplete EOF, stalled reads, or headers exceeding the cap receive a safe
400 response without consuming the OAuth callback. Complete denied callbacks
retain the readable error page; this parsing guard does not alter OAuth state
validation, PKCE, scopes, or credential storage.

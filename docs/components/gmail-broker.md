# Gmail credential broker

**Source:** `src/broker.rs`, `src/gmail.rs`, `src/mcp.rs`.

The native host broker, or the Docker-native broker container, accepts one
bounded JSON request per Unix-socket connection. It reads the persisted target
subject, rechecks connection/scopes/protected-store eligibility, refreshes a
token when necessary, and calls Gmail. Requests and responses are
newline-delimited and capped; malformed or oversized frames are rejected with a
generic error.

`GmailApi::list_emails` first lists message IDs, then fetches only metadata
headers (`From`, `Subject`, `Date`), labels, and the Gmail snippet. Snippets
are truncated to 300 Unicode characters and the response is bounded by the
caller’s hard page-size limit. The broker maps provider failures to stable
codes (`gmail_rate_limited`, `gmail_unavailable`, or
`reauthentication_required`) without forwarding upstream secrets or raw token
contents.

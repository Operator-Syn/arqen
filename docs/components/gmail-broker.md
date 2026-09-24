# Gmail credential broker

**Source:** `src/broker/`, `src/gmail/`, `src/mcp.rs`.

The native host broker, or the Docker-native broker container, accepts one
bounded JSON request per Unix-socket connection. It reads the persisted target
subject, rechecks connection/scopes/protected-store eligibility, refreshes a
token when necessary, and calls Gmail. Requests and responses are
newline-delimited and capped; malformed or oversized frames are rejected with a
generic error.

`GmailApi::list_emails` first lists message IDs, then fetches only metadata
headers (`From`, `Subject`, `Date`), labels, and the Gmail snippet. Snippets
are truncated to 300 Unicode characters and the response is bounded by the
caller’s hard page-size limit. `read_email` resolves the same persisted MCP
target, then fetches one `users/me/messages/{id}` resource with `format=full`;
the caller cannot supply another account. It decodes nested MIME text parts,
prefers plain text, converts HTML-only content, and does not download
attachments. The Gmail response is capped at 2 MiB, decoded body text at 256
KiB, and the serialized MCP result at 1 MiB. Oversized messages fail with
`message_too_large`, never as a claimed-complete truncation. The broker maps
provider failures to stable codes (`invalid_message_id`, `message_not_found`,
`message_too_large`, `gmail_rate_limited`, `gmail_unavailable`,
`credential_unavailable`, or `reauthentication_required`) without forwarding
upstream secrets, raw provider response bodies, or token contents. Credential
acquisition failures are kept distinct from Gmail API failures.

`list_labels` calls Gmail's `users/me/labels` endpoint through the same
selected-account and protected-credential flow. It returns Gmail's label IDs
unchanged with their display names and `system`/`user` types, including custom
labels. Agents select a record by name, then pass its ID explicitly to
`list_emails.label_ids` in a separate request. `list_emails` does not depend on
or call `list_labels`; its message `labels` remain Gmail IDs.

`create_label` and `delete_label` are separate broker operations and both
require the selected account's recorded `gmail.modify` scope before protected
credential access. Creation passes only the requested name to Gmail and returns
the typed `id`, `name`, and `type=user` record. Deletion receives the exact
label ID from an explicit earlier `list_labels` call; Gmail's labels.get call
checks only the requested label's `id,type`, rejects system labels, and does
not fetch or translate display names. The broker then issues labels.delete
with that unchanged ID and returns `{label_id,deleted:true}` only after success.
Gmail removes the deleted label from every associated message and thread but
does not delete messages. Provider status failures map to stable safe errors,
including `invalid_label_name`, `label_already_exists`, `invalid_label_id`,
`label_not_found`, `system_label`, rate limiting, reauthentication, and provider
unavailability; raw response bodies are discarded.

`mark_email_read` and `mark_email_unread` are separate broker operations. Each
resolves the persisted target, first requires its connected/read-only target
eligibility and then checks that target's recorded `gmail.modify` grant before
credential acquisition or a Gmail call. Missing local grant evidence returns
`insufficient_scope`; other Gmail 403 responses remain `gmail_unavailable`.
The Gmail client sends only the requested `UNREAD` addition or removal for one
message and asks for `id,labelIds`; it returns a typed `{message_id,is_read}`
result based on Gmail's response. No other message labels are changed or
returned.

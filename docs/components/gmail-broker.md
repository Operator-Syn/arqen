# Gmail credential broker

**Source:** `src/broker/`, `src/gmail/`, `src/mcp.rs`.

The native host broker, or the Docker-native broker container, accepts one
bounded JSON request per Unix-socket connection. It reads the persisted target
subject, rechecks connection/scopes/protected-store eligibility, refreshes a
token when necessary, and calls Gmail. Requests and responses are
newline-delimited and capped at 128 KiB per request and 4 MiB per response;
malformed or oversized frames are rejected with a generic error. The request
cap accommodates canonical UTF-8 serialization of maximum-size Unicode draft
bodies and headers without making transport allocation unbounded.

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

`create_label`, `apply_label`, and `delete_label` are separate broker operations
and require the selected account's recorded `gmail.modify` scope before
protected credential access. `apply_label` receives one message ID and one
exact user-label ID, checks that the label is custom, then adds it to that
message only. It distinguishes an unknown label from a missing message and
does not return provider response bodies. Creation passes only the requested
name to Gmail and returns the typed `id`, `name`, and `type=user` record.
Deletion receives the exact
label ID from an explicit earlier `list_labels` call; Gmail's labels.get call
checks only the requested label's `id,type`, rejects system labels, and does
not fetch or translate display names. The broker then issues labels.delete
with that unchanged ID and returns `{label_id,deleted:true}` only after success.
Gmail removes the deleted label from every associated message and thread but
does not delete messages. Provider status failures map to stable safe errors,
including `invalid_label_name`, `label_already_exists`, `invalid_label_id`,
`label_not_found`, `system_label`, rate limiting, reauthentication, and provider
unavailability; raw response bodies are discarded.

Label creation verifies missing provider type information with a GET for the
exact newly returned ID. It never assumes `user`, and invalid identities/types
fail closed. An unconfirmed successful write or transport failure returns
`gmail_unavailable` with may-have-succeeded guidance: read `list_labels` before
retrying. The broker must not repeat a POST merely because its verification GET
failed, including a GET 401. Label deletion accepts a successful empty body or
empty JSON object; unexpected bodies produce the same uncertainty guidance.
Internal uncertainty diagnostics retain only stage, status, and category,
never raw response bodies, tokens, or transport URLs.

`apply_label` maps malformed message and label IDs, missing messages or labels,
system labels, insufficient scope, reauthentication, rate limiting, and Gmail
unavailability to stable broker errors. It checks the selected account's
recorded `gmail.modify` scope before accessing protected credentials.

`mark_email_read` and `mark_email_unread` are separate broker operations. Each
resolves the persisted target, first requires its connected/read-only target
eligibility and then checks that target's recorded `gmail.modify` grant before
credential acquisition or a Gmail call. Missing local grant evidence returns
`insufficient_scope`; other Gmail 403 responses remain `gmail_unavailable`.
The Gmail client sends only the requested `UNREAD` addition or removal for one
message and asks for `id,labelIds`; it returns a typed `{message_id,is_read}`
result based on Gmail's response. No other message labels are changed or
returned.

`mark_email_for_deletion` and `delete_marked_email` are separate broker
operations. The first validates one message ID and stores a random marker
bound to the selected Google subject and exact message for 10 minutes; it does
not call Gmail. The second accepts only that marker, checks account and expiry,
and atomically consumes it before calling Gmail's messages.trash endpoint.
Missing, expired, replayed, or wrong-account markers fail with
`deletion_mark_required`. Gmail moves one message to recoverable Trash using
the existing `gmail.modify` grant. A failed call consumes its marker and
requires a fresh mark before retrying. Marker storage is process-local and is
cleared when the broker restarts.

Draft operations use Gmail's `users.drafts` API. New drafts contain one
recipient, subject, and text body; reply drafts derive their recipient,
subject, and thread from an existing message and require a caller-supplied body.
Draft listing returns separate draft and underlying message IDs. Draft IDs are
resolved to message IDs before action marks are registered, so draft send,
draft deletion, and existing message-to-Trash marks share one per-account,
per-message action slot. An explicit new mark replaces an opposite pending
mark; an in-flight action blocks transitions. Markers remain account-bound,
one-use, and 10-minute, process-local state.

Execution consumes the marker, then re-fetches the draft's underlying message
identity before mutation, including any refreshed-token attempt. A changed
revision requires a fresh mark and returns `action_mark_required` without
sending or deleting. Exclusivity includes the stable draft ID across edits.
This does not make the provider operation atomic: Gmail can still receive an
external edit between Arqen's preflight and its mutation by draft ID.

Draft metadata pages are all-or-nothing. A missing detail, failed response, or
invalid identity/labels aborts the page; no silently skipped partial result is
returned. Snippets are capped at 300 Unicode scalar values. Label/list/draft
JSON responses are capped at 2 MiB before deserialization, for both declared
and streamed body lengths. Public errors retain their stable categories and
never disclose provider payloads.

Internal `DraftListError` distinguishes list/detail stage, transport, provider
status, decoding, validation, size-limit, and configuration failures. It retains
only stage/category/status and a finite reason code, with a sanitized status
source for existing 401 refresh and public error mapping. It discards raw
decoding/transport errors, URLs, tokens, response bodies, and resource IDs.
At the broker's list-operation error boundary, one `arqen draft_list_failure`
line is written to service stderr after refresh handling finishes. It contains
only finite stage/category/reason values and the observed numeric HTTP status
(or `unobserved` before a response exists). Reasons distinguish missing required
fields, invalid JSON syntax/shape, invalid references, reference/detail mismatch,
empty identity fields, missing draft labels, and unsupported draft labels.
This diagnostic is internal, never part of the MCP result; do not enable raw
HTTP/payload logging to investigate a failure. A missing-field classification
does not by itself establish the exact absent field or the live response shape.
Public guidance reports that no partial page was returned; a detail 404 advises
retrying `list_drafts` instead of treating the incomplete page as an empty list.

Draft deletion uses Gmail's permanent `users.drafts.delete`; draft sending uses
`users.drafts.send` and reports uncertain provider outcomes without claiming
success. Custom labels remain organizational metadata, and system labels are
provider-managed. Label application and read-state changes reject DRAFT-labeled
messages so draft changes stay on the drafts resource boundary.

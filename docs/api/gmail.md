# Gmail upstream contract

**Source:** `src/gmail/`; external contract: [Gmail
`users.messages.list`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/list)
[`users.messages.get`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/get),
and [`users.messages.trash`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/trash).

Arqen uses the Gmail REST API with `users/me` after refreshing the selected
account’s token. A list request sends `q`, `maxResults`,
`includeSpamTrash`, repeated `labelIds`, and a `pageToken` when supplied. The
Gmail API can allow up to 500 messages per page; Arqen deliberately caps the
public tool at 50.

For each returned message ID, `list_emails` requests `format=metadata` and only
the `From`, `Subject`, and `Date` headers, labels, and snippet fields. Header
names are matched case-insensitively. A snippet is truncated to 300 Unicode
characters and the response marks whether truncation occurred. `read_email`
uses `users.messages.get` with `format=full` for that ID under the authenticated
selected account (`users/me`). It reads message headers and MIME parts, but does
not call the attachment download endpoint.

Gmail MIME body data is base64url-decoded. Nested parts are searched for
`text/plain` first; HTML-only messages are converted to readable text. Invalid
body encodings and malformed MIME trees are reported with a non-complete body
status. The full Gmail response is capped at 2 MiB, decoded body text at 256
KiB, and the serialized MCP result at 1 MiB. Oversized content returns the
stable `message_too_large` error rather than a truncated body.

`list_labels` calls `users/me/labels` and requests only each label's immutable
`id`, display `name`, and `type`. Gmail returns both system and user-created
labels. Arqen preserves the IDs exactly so callers can select a label by name
and pass its ID as `list_emails.label_ids` in a separate call.

`create_label` calls Gmail's
[`users.labels.create`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/create)
with `userId=me` and only a custom label `name`; the returned ID, exact name, and
verified `type=user` form the MCP result. A missing create-response `type` is
verified with `users.labels.get` for that exact returned ID, never synthesized.
Explicit null/unknown types or mismatched IDs/names fail closed. An unconfirmed
write returns a may-have-succeeded warning, not an automatic POST retry.
`delete_label` accepts the exact ID from a
separate `list_labels` call. It first calls `users.labels.get` requesting only
`id,type` to reject system labels, then calls
[`users.labels.delete`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/delete)
with `userId=me` and that same ID. Google's method reference describes an empty
JSON object on success; callers must not assume every successful HTTP deletion
has a JSON body. Arqen accepts zero bytes or `{}` after a successful status,
rejects other bodies, and preserves non-success status checks. Only a confirmed
successful response permits Arqen to return
`{label_id, deleted:true}`. Gmail's delete
operation removes the label from every message and thread using it but does not
delete the messages.

`apply_label` accepts a message ID from `list_emails` and a custom label ID
from `list_labels`. It calls
[`users.labels.get`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/get)
to confirm that the exact label ID belongs to a user label, then calls
[`users.messages.modify`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/modify)
on `users/me/messages/{messageId}/modify` with only
`addLabelIds: [labelId]`. It affects one message, not its thread, preserves
other labels, and returns the IDs plus `applied: true` only after Gmail's
response includes the label. System labels are rejected before the modify call.

The Gmail API method references list `gmail.modify` as an accepted scope for
label create, apply, and delete (along with `gmail.labels` and the broader
`mail.google.com`). Arqen already requests and checks the selected account's
recorded `gmail.modify` grant; this operation does not broaden OAuth scopes.
The documented `Label.name` schema requires a string but specifies no length or
character pattern. Arqen rejects blank names and control characters; Gmail
remains the authority for reserved-name conflicts and other provider name
rules. Gmail documents a 10,000-label mailbox maximum.

The read-state tools use Gmail's
[`users.messages.modify`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/modify)
method on `users/me/messages/{messageId}/modify`, with `addLabelIds: ["UNREAD"]`
or `removeLabelIds: ["UNREAD"]` and a response projection of `id,labelIds`.
They return only the message ID and whether Gmail's returned labels indicate
the message is read. The method requires `gmail.modify` (or the broader
`mail.google.com`); Arqen requests only `gmail.modify`, not `gmail.labels` or
`mail.google.com`.

Message deletion uses an explicit two-call guard. `mark_email_for_deletion`
does not call Gmail: it returns an opaque, account-bound, one-use marker for
the exact message ID, held in broker memory for 10 minutes. A separate
`delete_marked_email` call must supply that exact marker; the broker checks the
selected Google subject, expiry, and single-use state before consuming it.
After consumption, Arqen calls Gmail's
[`users.messages.trash`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/trash)
at `POST users/me/messages/{messageId}/trash`, requesting only the response ID.
This moves one message to recoverable Trash, not permanent deletion, and uses
the existing `gmail.modify` grant. Failed calls consume their marker and
require a fresh mark before retrying.

Draft operations use Gmail's `users.drafts` resource. `list_drafts` lists draft
IDs, then fetches bounded metadata for each result; it keeps Gmail's draft ID
distinct from the message ID contained by that draft. The page fails as a whole
if a draft disappears mid-page or any metadata response fails: it never silently
skips a detail and reports a complete page. Metadata must correlate with the
listed draft ID, contain nonempty message/thread IDs and only `DRAFT` labels.
Snippets use the existing Unicode-safe 300-character cap. All label/list/draft
JSON responses are capped at 2 MiB, including chunked responses.
`create_draft` builds a plain-text MIME message with one recipient, subject,
and body (at most 24,576 Unicode scalar values), then posts it to
`users.drafts.create`. `create_reply_draft` reads the source message headers,
derives its reply address and subject, supplies the source thread ID and reply
headers, and creates a separate draft. Draft messages are provider-managed and
cannot receive labels other than Gmail's `DRAFT` system label.

Draft deletion and sending each use a dedicated mark tool and marker-only
execution tool. The broker resolves a draft ID to its contained message ID and
uses that message identity to enforce one pending destructive action across
draft send, draft deletion, and message-to-Trash. A new mark replaces a pending
opposite mark; an executing action blocks a new mark. The broker consumes a
marker before making the provider call and rechecks the underlying message ID
against the marked revision. Edited drafts fail closed and require fresh marks;
opposite marks remain exclusive even when the draft's message ID changes.
An external edit after preflight can still race Gmail's draft-ID-based mutation;
there is no atomic provider compare-and-swap. Draft deletion uses
`users.drafts.delete`, which is permanent; sending uses `users.drafts.send`.
If send outcome cannot be confirmed, the caller must check whether the draft
remains before retrying. Mark state is process-local and expires after 10
minutes.

`apply_label`, `mark_email_read`, `mark_email_unread`, and message-to-Trash
inspect the message labels and reject messages carrying `DRAFT`; use the
draft-specific operations for draft messages. Custom labels are organizational
metadata, never pending action flags or authorization for a destructive
operation.

The selected account's recorded OAuth grants must contain
[`gmail.readonly`](https://developers.google.com/workspace/gmail/api/auth/scopes)
for target eligibility; `apply_label`, the two read-state tools, message-to-Trash,
and draft creation, deletion, and sending additionally require `gmail.modify`. Both scopes are
restricted. Google's `gmail.modify` description
includes reading, composing, and sending email. After Arqen begins requesting
that new scope, the selected account must be reauthorized to record the actual
grant; a refresh of its existing token does not retroactively add the scope.
For external OAuth apps in Testing, Google expires refresh tokens after seven
days when Gmail scopes are requested. Review the current Gmail scope policy
before any public deployment.

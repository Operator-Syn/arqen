# Gmail upstream contract

**Source:** `src/gmail/`; external contract: [Gmail
`users.messages.list`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/list)
and [`users.messages.get`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/get).

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

The read-state tools use Gmail's
[`users.messages.modify`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.messages/modify)
method on `users/me/messages/{messageId}/modify`, with `addLabelIds: ["UNREAD"]`
or `removeLabelIds: ["UNREAD"]` and a response projection of `id,labelIds`.
They return only the message ID and whether Gmail's returned labels indicate
the message is read. The method requires `gmail.modify` (or the broader
`mail.google.com`); Arqen requests only `gmail.modify`, not `gmail.labels` or
`mail.google.com`.

The selected account's recorded OAuth grants must contain
[`gmail.readonly`](https://developers.google.com/workspace/gmail/api/auth/scopes)
for target eligibility; the two read-state tools additionally require
`gmail.modify`. Both scopes are restricted. Google's `gmail.modify` description
includes reading, composing, and sending email. After Arqen begins requesting
that new scope, the selected account must be reauthorized to record the actual
grant; a refresh of its existing token does not retroactively add the scope.
For external OAuth apps in Testing, Google expires refresh tokens after seven
days when Gmail scopes are requested. Review the current Gmail scope policy
before any public deployment.

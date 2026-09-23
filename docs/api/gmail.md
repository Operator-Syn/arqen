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

The configured OAuth grant must contain
[`https://www.googleapis.com/auth/gmail.readonly`](https://developers.google.com/workspace/gmail/api/auth/scopes).
Google classifies this as a restricted scope; public deployment therefore
requires the operator to review Google’s current verification and user-data
policy requirements before exposing the service.

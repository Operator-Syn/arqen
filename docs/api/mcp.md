# MCP HTTP API

**Status:** `verified-repository` for the route, auth, and tool implementation;
`verified-external` for the transport model in the [MCP transport
specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports).

## Endpoint and authentication

- URL: `POST /mcp` (the reverse proxy supplies public HTTPS).
- Authentication: `Authorization: Bearer <configured-token>`; the token is
  compared as a constant-time exact value.
- Host and Origin: explicit allowlists are required through
  `ARQEN_MCP_ALLOWED_HOSTS` and `ARQEN_MCP_ALLOWED_ORIGINS`. Missing Origin is
  accepted by the SDK; a supplied Origin must match.
- Request body: maximum 1 MiB.
- Liveness: `GET /healthz` with the same bearer token returns `204 No Content`
  when the HTTP process is running.
- Readiness: `GET /readyz` with the same bearer token returns `204 No Content`
  only when the broker, account database, selected target, and configured
  protected credential are usable. It returns `503` with a stable
  `{code,message}` JSON body otherwise. It does not call Gmail or refresh a
  token.

The MCP SDK negotiates the protocol version and may return JSON or a
request-scoped SSE stream according to the request and response needs. Clients
should send `Content-Type: application/json` and an `Accept` value that allows
`application/json` and `text/event-stream`.

## Tool: `list_emails`

The tool accepts a JSON object; every argument is optional. Omit `query` (or
send it as `null`) to use `in:inbox`. The page size defaults to 20. A
`next_page_token` from one response can be supplied as `page_token` to fetch the
next page.

| Argument | JSON type | Optional/default and constraints |
| --- | --- | --- |
| `query` | string or null | Omitted, `null`, or blank uses `in:inbox`; Gmail search syntax, at most 1,024 characters and no control characters |
| `label_ids` | array of strings | Omitted defaults to `[]`; at most 20 IDs, each 1–256 characters and no control characters |
| `max_results` | integer | Omitted defaults to 20; valid values are 1–50 inclusive |
| `page_token` | string or null | Omitted or `null` starts at the first page; otherwise pass the previous response's `next_page_token`; 1–4,096 characters and no control characters |
| `include_spam_trash` | boolean | Omitted defaults to `false`; controls whether Gmail includes Spam and Trash |

The server always applies these arguments to the one target selected in the
Arqen TUI. It does not accept a Google subject or email as a tool argument.

Successful results contain `target_email`, `messages`, `next_page_token`, and
`result_size_estimate`. Each message contains its Gmail `id`, `thread_id`,
`from`, `subject`, `date`, `labels`, `snippet`, and `snippet_truncated`. Full
message bodies and attachments are intentionally outside this milestone.
`next_page_token` is `null` when no further page is available; otherwise pass
it unchanged as `page_token`. Optional header fields (`from`, `subject`, and
`date`) may be `null` when Gmail did not return those headers.

Invalid argument values return `invalid_request` with the relevant validation
constraint in the message. For example, `max_results` outside 1–50 reports
`max_results must be between 1 and 50`.

## Failure codes

The broker uses these stable codes: `invalid_request`,
`target_not_configured`, `target_unavailable`, `reauthentication_required`,
`credential_unavailable`, `gmail_rate_limited`, `gmail_unavailable`, and
`internal`. `credential_unavailable` means the broker could not access the
protected refresh credential or obtain an access token; `gmail_unavailable`
means the Gmail mail-list request failed. A failure never includes an access
token, refresh token, or raw provider response body.

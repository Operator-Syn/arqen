# Arqen MCP audit remediation — 2026-10-04

**2026-10-05 follow-up:** The historical DRAFT-only validation claim below was
incorrect. Approved live diagnostics on revision
`f8ca4d53571910bd00f14367f9321c3fe3cbf228` (0.1.12) captured HTTP 200 draft
details with `[DRAFT, IMPORTANT]` in separate native `max_results=1` and
`max_results=50` windows. The current source requires DRAFT presence rather
than exclusivity; temporary label diagnostics have been removed. See the
[current Gmail contract](../api/gmail.md) and [regressions](../development/testing.md).
This follow-up does not claim deployed or live-verified repair.

## Outcome and verification boundary

Tested source changes are implemented on `main`, based on
`102bbce4c26d075bbc85ac0a8511c254ec66a204` (Cargo 0.1.10). The initial working
tree was clean. Nothing was committed, pushed, deployed, or changed in OAuth
scopes, credentials, global MCP configuration, or account selection.

**This is not a completed live remediation.** Label response defects and draft
revision/schema/transport/callback defects are source-fixed and locally tested,
not live-verified. The actual live `list_drafts` failure remains unresolved;
listing hardening and sanitized diagnostic classification do not establish its
cause or repair it by themselves.

The native registered product `list_drafts(max_results=1)` reproduced
`gmail_unavailable: Gmail could not list drafts` during this session. A subsequent
native `list_labels` succeeded. No new live mailbox mutation was performed, so
there are no new Gmail fixtures or cleanup artifacts. The earlier authorized
email was not resent, and existing messages/drafts were untouched.

At 19:46 +08:00, image metadata was read from the exact images used by the running
broker and MCP containers, not merely mutable image tags. Both remained 0.1.9,
revision `757db1a1d260dae7ff3f45184d0a80e0dbfff304`. Tests and the local build do
not modify those running processes.

## Confirmed defects versus unresolved hypotheses

| Area | Confirmed evidence and change | Unresolved/live boundary |
| --- | --- | --- |
| Label deletion | The old parser rejected zero-byte successful responses. RED regression reproduced EOF parsing; new parser accepts empty 200/204 and documented `{}`, rejects other nonempty bodies, and preserves HTTP errors. | The exact historical live status/body was never captured. An empty body is a plausible cause, not established live evidence. |
| Label creation | Successful-response parsing/validation could report failure after a committed POST. Missing type is now verified by one GET for the exact returned ID; ID/name/user type are checked, never invented. Ambiguous writes return may-have-succeeded guidance. | Missing `type` in the historical response remains a hypothesis. No new label was created live. |
| Draft listing | Confirmed missing reference/detail correlation and snippet cap; malformed references are now rejected before detail fetch, detail IDs must correlate, metadata must affirm only DRAFT, snippets are capped at 300 Unicode scalars. Pages remain all-or-nothing. | The native failure reproduced, but the list/detail/status/decoding/validation boundary is unavailable in the old running error output. The documented projection is supported; no projection defect was established. |
| Draft authorization | Source execution previously used a stable draft ID without rechecking the marked message revision. Edited-revision regression captured an unauthorized send before the fix. Execution now rechecks identity before mutation and rejects edits. | No live exploit was demonstrated. External edits can still race the provider GET-to-mutation gap. |
| Schema/runtime | Reply ID schema lacked runtime ASCII constraints; list inputs were not consistently closed; Unicode character schemas conflicted with byte validators; draft-ID categories and reply-source guidance diverged. | Local HTTP schema/handler tests do not establish any native client's pre-invocation validation. |
| Unicode transport | Review found newly accepted multibyte draft inputs exceeded the old 64 KiB frame. Actual broker-reader RED test failed; bounded 128 KiB cap now accepts canonical maximum-size inputs and still rejects oversize frames. | Transport bounds remain bytes, independent of character limits. |
| Callback | Deterministic partial `GET ` caused a premature HTTP response because one TCP read was treated as a complete request. Headers now accumulate through CRLF-CRLF within the existing 8192-byte cap and two-second read timeout. | This confirms a defect consistent with the historical flake; no captured historical packet trace proves it caused that exact first failure. Live browser/OAuth behavior was not tested. |

### Provider documentation checked

Current Google method references were retrieved directly after the extraction
backend returned 403:

- [labels.create](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/create): successful response is a newly created Label.
- [labels.delete](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/delete): prose still says an empty JSON object; Gmail discovery has no delete response schema. This does not prove the audited body was empty.
- [labels.get](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.labels/get): Label response used for exact-ID verification.
- [drafts.list](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.drafts/list) and [drafts.get](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.drafts/get): metadata format and pagination supported.
- [Draft guide](https://developers.google.com/workspace/gmail/api/guides/drafts): draft message identity changes when replaced; DRAFT remains provider-managed.

## Explicit contract changes

- All 17 emitted input schemas are closed objects. `list_labels` accepts only
  empty arguments; `list_emails` no longer ignores unknown fields or selectors.
- Reply source `message_id` advertises 1–256 ASCII letters/digits/hyphens/underscores.
- Text limits count Unicode scalar values, matching schema lengths. Draft body
  changes deliberately from 24 KiB to 24,576 characters; subject allows 998 and
  recipient 320 characters. HTTP/frame/provider/readable-body bounds remain bytes.
- Broker request cap increases from 64 to 128 KiB to carry maximum canonical
  Unicode draft inputs; response cap remains 4 MiB.
- Label/list/draft provider JSON is bounded at 2 MiB, including chunked bodies.
- Malformed draft IDs use existing `invalid_request`, replacing MCP-only
  `invalid_draft_id`; reply-source not-found guidance points to `list_emails`.
- Label uncertain outcomes retain `gmail_unavailable` but explicitly warn that
  the write may have succeeded. A failed verification GET, including 401, cannot
  trigger replay of the committed POST. Ordinary rejected-write 401 refresh
  behavior remains distinct from uncertain post-commit verification.
- Draft pages fail rather than silently omit missing/invalid details. Detail 404
  returns existing `message_not_found` with page-retry guidance. Other errors keep
  their existing categories and safe all-or-nothing explanations.
- Internal label uncertainty and `DraftListError` retain sanitized stage/status/
  category only. Raw transport/decoding errors, bodies, tokens, URLs, and resource
  identifiers are not exposed through these diagnostics. No logging was added.
- Draft marks remain account-bound, expiring, single-use, and mutually exclusive.
  Exclusivity also uses stable draft ID across revisions. Consumed markers are
  finished after preflight/provider failure and require fresh marking.

Composition remains explicit: returned label/message IDs feed separate calls;
returned draft IDs feed mark calls, and exact markers feed execute calls. There
is no implicit account switching or display-name-to-ID guessing.

## RED→GREEN and check outcomes

Meaningful failing regressions preceded production fixes for empty label delete,
missing-type verification, created-label identity mismatch, response bounds,
draft correlation/reference/label/snippet checks, revision authorization and
opposite-action transitions, reply-source guidance, schema closure and ID bounds,
Unicode semantics, draft-ID error category, Unicode transport capacity, callback
fragmentation, and disappearing-detail/status diagnostic guidance.

The exact forwarding tests were additionally exercised with temporary operation,
argument, and output mutations; those mutations failed the assertions and were
restored. Passing schema tests inspect actual local HTTP `tools/list` output.

Final parent-run checks after all workers finished:

| Command | Outcome |
| --- | --- |
| `nix develop .#arqen --command cargo fmt --check` | Passed |
| `nix develop .#arqen --command cargo test --locked` | Passed: 133 library, 95 binary, 0 doc tests; no failures |
| `nix develop .#arqen --command cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `nix develop .#arqen --command cargo build --locked` | Passed |
| `nix flake check --no-build` | Passed for x86_64-linux; incompatible systems omitted, not checked |
| `python3 -m unittest discover -s tests/python` | Passed: 7 tests |
| Prescribed Nix shellcheck of bootstrap/up/smoke scripts | Passed |
| `git diff --check` | Passed |
| Parent focused callback suite | Passed: 15 tests |
| Parent actual broker-frame regressions | Passed: 2 tests after meaningful RED |

The final Rust suite has 228 passing tests, 50 more than the historical 178-test
baseline. This is not a claim that every new assertion independently reproduces
a historical live defect.

Initial/intermediate failures are not erased by the final pass:

- Historical full audit run had a callback failure before focused/full retries.
- Callback investigation reproduced 2 failures in 100 denied-callback runs;
  deterministic fragmentation then failed before the fix. Worker post-fix runs:
  denied callback 20/20 and fragmentation 10/10 passed.
- Concurrent work temporarily caused compilation failures and formatting/Clippy
  failures while files were being edited; final combined checks passed.
- Oversized-response fixtures initially hit BrokenPipe; fixtures now account for
  intended bounded-client disconnects instead of hiding unrelated panics.
- One diagnostic fixture initially hung because a supposed valid detail omitted
  required DRAFT metadata; fixture corrected and bounded accept deadline added.
- An initial exact callback filter selected zero tests; the corrected invocation
  selected the actual regression and demonstrated RED.
- Parent first integration formatting check failed on still-in-progress diagnostic
  files; final prescribed check passed after integration.

## Status matrix — all 17 tools

Every row has a passing local emitted-schema audit. “Historical” refers to the
provided 2026-10-04 audit, not a new verification of this source revision.

| Tool | Mocked/local evidence in final suite | Live evidence / remaining status |
| --- | --- | --- |
| list_emails | Metadata/provider, HTTP forwarding, closed input | Historical success; new source not live-verified |
| read_email | MIME/bounds/errors, HTTP forwarding | Historical success; new source not live-verified |
| list_labels | Provider and HTTP forwarding; closed empty input | Native read succeeded this session on old runtime |
| create_label | Creation, exact-ID type verification, uncertainty, limits | Historical false negative; source-fixed handling, not live-verified |
| apply_label | Provider protections and exact list→apply composition | Historical success; new source not live-verified |
| delete_label | Empty success, strict invalid bodies/status, explicit ID composition | Historical false negative; source-fixed handling, not live-verified |
| mark_email_read | Provider state/protections and distinct HTTP forwarding | Historical success; new source not live-verified |
| mark_email_unread | Provider state/protections and distinct HTTP forwarding | Historical success; new source not live-verified |
| mark_email_for_deletion | Account/marker rules and exact mark→Trash composition | Historical success; new source not live-verified |
| delete_marked_email | Trash framing/protections and exact marker forwarding | Historical success; new source not live-verified |
| list_drafts | Empty/paginated/invalid/disappearing/bounded pages; typed output; sanitized stages | Native failure reproduced on old runtime; actual root cause unresolved |
| create_draft | MIME construction, Unicode broker transport, exact operation/output | Historical success; new source not live-verified |
| create_reply_draft | Source/header/thread handling, source errors, schema/transport/forwarding | Historical success; new source not live-verified |
| mark_draft_for_deletion | Revision/account/expiry/opposite-action rules, exact forwarding | Historical success; strengthened source not live-verified |
| delete_marked_draft | Unchanged/edited revision preflight, single use, exact forwarding | Historical success; strengthened source not live-verified |
| mark_draft_for_sending | Revision/account/expiry/concurrent transition rules, exact forwarding | Historical success; strengthened source not live-verified |
| send_marked_draft | Unchanged/edited preflight, marker concurrency and exact output | Historical success and recipient-confirmed delivery; not resent; strengthened source not live-verified |

## Changed files

Production:
`src/broker/{actions,mod,protocol}.rs`,
`src/broker/handlers/{drafts,errors}.rs`, `src/callback/pages.rs`,
`src/gmail/{client,mod,models,responses}.rs`, `src/gmail/client/drafts.rs`,
`src/server/{mod,routes}.rs`.

Tests:
`tests/unit/{broker,broker_drafts,broker_frames,callback,gmail}.rs`,
`tests/unit/server/{mod,draft_tools,composition,contracts,fixtures,schemas}.rs`.

Documentation:
`docs/api/{gmail,mcp}.md`,
`docs/components/{gmail-broker,mcp-server,oauth-and-keyring}.md`,
`docs/development/testing.md`, and this audit handoff.

Module responsibilities were preserved: provider parsing/verification stays in
Gmail adapters, authorization in broker actions/handlers, schemas in request
models/server routes, and stream framing in its protocol/callback boundary.
No new dependency or unrelated refactor was introduced.

Code-knowledge MCP was available as `workspace-project`. Final exact-path checks
reported no recorded coverage gaps and metadata matches for operated source/test
paths; this is best-effort coverage, not proof of exhaustive graph completeness.
Direct source and executed checks remain authoritative.

## Remaining operator decisions and safety limits

1. Approve review/commit/deployment separately. No such approval is assumed.
2. Rebuild and deploy the changed broker and MCP together before judging the new
   public contracts; deploy the app runtime for the callback fix. Restart clears
   process-local action marks. No credential rotation or scope change is needed.
3. Invoke the actual registered `list_drafts` after approved deployment. If it
   still fails, inspect the new sanitized internal stage/category/status through
   an approved diagnostic path; do not dump provider bodies or credentials.
4. Request fresh, uniquely named, fixture-specific permission before live label,
   draft, send, deletion, read-state, or Trash tests. Read back every mutation,
   including error outcomes, before any retry; verify cleanup.
5. Preserve the residual Gmail race: a preflight message-ID check cannot make a
   stable-draft-ID mutation atomic. No compare-and-swap or content-lock guarantee
   is claimed.

Priority B's actual live repair and changed-runtime verification remain open.
Local passing mocks are not substituted for those missing results.

## Focused list-drafts investigation — 2026-10-05

The clean checkout and exact running broker/MCP image IDs were checked at
revision `15242b5480aed196bca17d0de1fefe46bc55502b`, version 0.1.10. The native
registered `list_drafts(max_results=1)` again returned the invalid-draft-page
error. No mailbox mutations, account/scopes/credential/configuration changes,
commit, push, or deployment were performed.

The uncommitted diagnostic patch adds finite invariant reasons and one sanitized
broker stderr line at the list-operation failure boundary. Public responses,
metadata projection, DRAFT validation, identity correlation, response/snippet
bounds, and all-or-nothing behavior remain unchanged. The synthetic diagnostic
regression failed before implementation because the error had no reason; it
passed afterward, with 19 focused listing tests passing. This is RED→GREEN for
diagnostic capability, **not a causal regression or a live-listing repair**.

Current Google drafts.get and draft-guide documentation were fetched directly
after the extraction backend returned 403. They support the draft resource API
and describe only DRAFT labels on draft messages. Neither establishes the live
failing response shape, so no assumption was relaxed.

Capturing the live boundary through the registered tool requires separately
approved deployment of the diagnostic broker build. Until then, the exact cause,
causal fixture, production repair, and changed-runtime normal/paginated/max-50
verification remain blocked/unresolved. Existing drafts must remain untouched.

Local diagnostic-build checks:

| Command | Result |
| --- | --- |
| `nix develop .#arqen --command cargo fmt --check` | Passed |
| `nix develop .#arqen --command cargo test --locked` | Passed: 135 library, 95 binary, 0 doc tests |
| `nix develop .#arqen --command cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `nix develop .#arqen --command cargo build --locked` | Passed |
| `git diff --check` | Passed |

The first diagnostic RED invocation timed out while downloading the Nix
environment, not while executing the regression. Its retry selected one test
and failed on the missing reason field as expected; the post-change invocation
selected that same test and passed. Full local checks do not verify live Gmail.

### Separate client-schema check

A fresh temporary connection using installed Hermes native discovery retrieved
17 live tool schemas and was disconnected without business-tool calls. All 17
input objects were closed. `list_drafts` matched current source: page size 1–50,
default 20; nullable page token 1–4096 characters with the control-character
pattern; no account selector or extra properties. The current agent-facing
`tool_describe` also matched, with nullable strings normalized to `nullable:true`.
No current list-drafts metadata staleness was observed. This does not diagnose
the mailbox failure or prove a previous session's cached schema was refreshed.

The supported [Hermes MCP reload procedure](https://hermes-agent.nousresearch.com/docs/user-guide/features/mcp)
is `/reload-mcp` or a new session. Installed CLI source confirms that explicit
reload asks for approval, disconnects all MCP connections (not only Arqen),
rereads configuration, reconnects/discovers, and refreshes the agent tool
snapshot. `hermes mcp test arqen` is an isolated connectivity/discovery probe,
not a refresh of an existing session. No reload or configuration edit was made.

### Publication preparation and review correction

Following explicit commit/push authorization, the checkout fast-forwarded over
the automated `7bb127bfa273d43a20b3015fef925d9b114c019b` version bump to 0.1.11.
The first independent review found that finite validator errors unintentionally
changed the public Rust draft-creation error. The shared creation boundary now
restores the original message-only `Gmail returned an incomplete draft` error.
Two synthetic regressions covering empty draft/message/thread IDs in new and
reply creation failed before that correction and passed afterward. Listing
fixtures now assert the exact unchanged public code and wording.

Final parent checks after correction on 0.1.11 passed: prescribed Nix formatting,
locked tests (137 library, 95 binary, 0 doc tests), locked all-target/all-feature
Clippy with warnings denied, and locked build. No changed runtime has been
activated or tested. The intended publication remains diagnostic-only, not a
confirmed repair of normal-mailbox listing.

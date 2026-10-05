# Test layout

Keep test code under the repository's root `tests/` directory:

- `tests/unit/` contains Rust unit modules grouped by their owning `src/`
  module. Source modules attach them with `#[cfg(test)]` and `#[path]`, which
  keeps tests able to inspect private implementation details.
- `tests/python/` contains Python `unittest` suites. Run them with
  `python3 -m unittest discover -s tests/python`.
- `tests/integration/` is reserved for black-box suites organized by language
  or system boundary. Declare Cargo integration targets with paths under this
  directory so their source stays categorized here.

Add tests under the closest matching subtree. Keep test source out of `src/`
and avoid loose test files at the root of `tests/`. `cargo test` remains the
Rust test entry point and discovers the centralized unit modules through their
source owners.

## Gmail Trash request framing

Run `cargo test message_trash` for the provider-level Trash regressions in
`tests/unit/gmail.rs`. The local mock HTTP server exercises the production
draft-check GET followed by the Trash POST and captures the HTTP/1.1 request.
The empty POST must send `Content-Length: 0`, with no payload or transfer
encoding; keep the explicit empty body in the provider client. The tests also
cover draft protection and rejection of an unexpected response message ID.
These tests use synthetic tokens and do not prove deployment, Google grants,
or authenticated Gmail behavior.

## MCP audit regressions

Run focused checks in the pinned environment before the full locked suite:

```bash
nix develop .#arqen --command cargo test --locked --lib priority_a_label_
nix develop .#arqen --command cargo test --locked --lib draft_listing_
nix develop .#arqen --command cargo test --locked --lib broker
nix develop .#arqen --command cargo test --locked --bin arqen server::tests
```

`tests/unit/gmail.rs` exercises empty label deletion responses, strict
create-response identity/type verification, uncertain-write handling without
POST retries, bounded declared/chunked JSON, draft metadata pagination,
malformed references/details, disappearing drafts, and Unicode-safe snippets.
HTTP fixtures must tolerate early disconnects only where bounded-response
rejection intentionally closes an oversized response; never swallow unrelated
fixture panics.

The shared single/multi-response Gmail mocks have five-second accept deadlines
and two-second read/write timeouts. Draft-list diagnostic fixtures also bound
accept/read waits and use synthetic resources, not copied mailbox metadata.
`draft_listing_diagnostics_identify_invariants_without_private_values` checks
finite internal reasons, the actual 201 detail status, unchanged public wording,
and absence of private values/raw serde sources. Minimal metadata is accepted
without optional headers, payload, or snippet. These diagnostic tests do not
reproduce or repair the unresolved normal-mailbox failure: capture its actual
boundary after an approved diagnostic deployment, then add a causal regression.

`tests/unit/broker_drafts.rs` and broker action tests cover unchanged/edited
draft revisions, target changes, consumed/expired markers, and concurrent
opposite-action transitions. Provider preflight is not an atomic mutation.

`tests/unit/server/` checks all 17 schemas emitted by HTTP `tools/list`, not just
Rust declarations. Scripted Unix-socket fixtures assert the exact operation and
arguments, and compare complete typed MCP results. Composition tests pass exact
returned message/label IDs and markers into separate calls. Unicode boundary
tests use scalar-value counts; transport caps remain bytes.
`tests/unit/broker_frames.rs` runs maximum-size multibyte draft requests through
the actual bounded broker reader and verifies oversized-frame rejection. Keep
this transport regression when changing content limits; validators alone do
not establish that accepted inputs can cross the broker boundary.

These fixtures demonstrate local provider/broker/HTTP behavior only. They do
not establish a native client's schema rejection or repaired deployed Gmail
behavior. Live mutation tests require new fixture-specific permission. Never
reuse existing mail or drafts as disposable fixtures, and read back every
approved mutation, including writes that report failure, before retrying.

## Callback fragmentation regression

Run `nix develop .#arqen --command cargo test --locked callback::tests` for the
loopback listener checks. Fragmented requests must not receive a response before
the header terminator arrives, including a terminator split across TCP writes.
The regression uses bounded socket reads, not arbitrary sleeps. EOF, read
timeout, over-cap rejection, exact-cap acceptance, and denied-page readability
are tested separately. Channel receives are bounded instead of polling with
sleeps. Passing these tests does not establish live browser/OAuth behavior.

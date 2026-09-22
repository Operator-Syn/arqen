# Code modularization audit

**Baseline:** `9047f83` (2026-09-22). **Scope:** checked-in Rust source under
`src/`; deployment, activation, live OAuth/Gmail, and live graphical behavior
were excluded. **Confidence:** `verified-repository` for direct source and
local tests; graph coverage is best-effort.

## Baseline findings

The baseline mixed entrypoint, state, rendering, protocol, persistence, and
adapter concerns in large files. The largest files were `src/main.rs` (1,841
lines), `src/ui/mod.rs` (1,141), `src/control.rs` (1,112),
`src/ui/accounts.rs` (977), `src/broker.rs` (811), `src/ui/chrome.rs` (786),
`src/lib.rs` (600), `src/callback.rs` (571), `src/server.rs` (550),
`src/gmail.rs` (509), `src/auth.rs` (503), and `src/secrets.rs` (455).

The structural graph prioritized `handle_mouse`, `handle_event`,
`render_account_details`, `callback_thread`, `proxy_http`, and broker handlers.
The graph reported `src/control.rs` as parse-partial for its full range; direct
source inspection was therefore authoritative for that boundary. A reported
`App.new`/`AccountStore.list_accounts` cycle was not treated as a production
recursion claim without direct-source support.

## Resulting boundaries

- Binary startup is now in `src/main.rs`, `src/cli.rs`, and `src/config.rs`.
- TUI state, application transitions, browser lifecycle, input routing, and
  terminal runtime live under `src/tui/`.
- UI layout, interaction, account panels, chrome, dialogs, and modal geometry
  are split under `src/ui/` while preserving the existing render contracts.
- OAuth, broker, callback, control gateway, Gmail, secrets, server, and SQLite
  responsibilities are grouped under directory modules with stable facades.
- `src/lib.rs` re-exports the existing account/store API; no public contract or
  persistence schema was intentionally changed.

## Conventions applied

The refactor follows the canonical rules in
[`docs/development/code-organization.md`](../development/code-organization.md):
single responsibility, typed composition, silent internals, early validation,
effect isolation, explicit facades, and colocated tests. The roughly 200-line
value is a recommendation for review, not an absolute ceiling or pass/fail
gate; cohesive rendering and
test fixtures may remain larger when splitting would obscure the behavior. The
remaining production exceptions are `src/ui/accounts/details.rs`, whose
single renderer maintains one shared vertical scroll model, the footer/list
renderer files that keep layout arithmetic together, and the small control
facade that owns shared gateway state/constants. Their high-level callers and
side effects are already isolated; a future change that adds another reason to
change should split them further.

## Verification boundary

Source-level verification covers formatting, compilation, unit/regression tests,
clippy, and build checks. It does not establish activation, deployment, live
Google consent, keyring/OpenBao availability, browser behavior, or graphical
session health. A post-refactor full graph index found 173 source/module files
and one best-effort parse gap in `src/control/tests.rs` (lines 1-379); the
direct source and passing tests remain authoritative for that range. The graph
also reported a `cli.run` helper cycle that direct source inspection did not
confirm as recursive production control flow. Documentation mockup images are
ignored by design. Re-run the codebase index after future structural changes
and inspect any flagged coverage ranges directly.

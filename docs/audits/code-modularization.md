# Code modularization audit

**Baseline:** `9047f83` (2026-09-22). **Scope:** checked-in Rust source under
`src/`; deployment, activation, live OAuth/Gmail, and live graphical behavior
were excluded. **Confidence:** `verified-repository` for direct source and
local tests; graph coverage is best-effort.

## Follow-up audit (2026-10-01)

**Inspected revision:** `8603f1d`, with the current label tools, two-step
deletion tools, and modularization changes included in the source review. The
repository-scoped module audit script was available. The MCP refresh initially
rejected the host checkout path; rerunning through the repository's isolated
Docker launcher with `/workspace/project` refreshed the current source.
Deployment files, generated/ignored state, and live account/runtime behavior
were kept outside the Rust module refactor because they are separate
operational surfaces rather than application modules.

### Changes made

- `src/broker/handlers.rs` is now a small facade. `emails.rs`, `labels.rs`, and
  `target.rs` own their respective operation and eligibility rules;
  `tokens.rs` owns credential refresh/cache composition; `errors.rs` owns
  provider-to-broker error translation. This gives each behavior a local
  change boundary while keeping the broker operation names and response
  conversion explicit from `protocol.rs`.
- `src/control/login_rate_limit.rs` owns the failed-login window and retry
  counter. Gateway state continues to own the limiter instance, while HTTP
  authentication calls its narrow policy interface.
- `tests/unit/mcp.rs` now holds broker wire-contract tests separately from the
  serialized request, validation, error, and response types in `src/mcp.rs`.
- `tests/unit/server/mod.rs` keeps shared server-test setup and the HTTP surface
  check; list/read, label, message-state/input-validation, and readiness tests
  now live in focused child test modules. This removes the former 1,141-line
  mixed test file without changing the HTTP or MCP contracts.

These splits preserve typed requests and results, use ordinary `Result` and
explicit broker responses between stages, add no incidental output, and retain
early validation and fail-closed target/scope checks. They do not change public
tool names, wire fields, account selection, OAuth scopes, or persistence.

### Remaining review signals

The audit script still flags `src/broker/client.rs` (674 lines),
`src/broker/handlers/errors.rs` (289), `src/broker/handlers/tokens.rs` (256),
`src/gmail/client.rs` (556), `src/gmail/responses.rs` (305), `src/mcp.rs`
(306), `src/secrets/openbao.rs` (240), `src/server/routes.rs` (230),
`src/tui/flow.rs` (213), `src/tui/input.rs` (201),
`src/ui/accounts/details.rs` (313), `src/ui/accounts/list_render.rs` (216),
and `src/ui/chrome/footer_actions.rs` (228). The 200-line value is a review
marker, not a threshold. The Gmail
client, response decoder, and OpenBao adapter each have one provider-boundary
purpose. The account-details and list renderers keep one shared layout/scroll
contract together; footer actions keep one navigation component together.
`broker/client.rs` combines typed operation mapping with bounded Unix-socket
framing, so it remains the clearest candidate for a later transport extraction
if a change makes that boundary more complex. `handlers/errors.rs` is 289 lines
but remains one stable error-translation boundary with operation-specific
codes/messages.

Test files are centralized under the root `tests/` directory, with Rust unit
modules grouped by source owner, Python tests separated by language, and an
integration-test scaffold documented for future suites. Rust unit modules are
attached to their owning source modules by path so tests retain access to
private implementation details. The former production `include!` fragments
have been replaced with child modules and explicit facade imports/re-exports;
`include_str!` remains only where source assets are embedded as data.

### Verification

`cargo fmt --check`, `cargo check --all-targets`, strict Clippy,
`cargo build`, `nix flake check --no-build`, and `git diff --check` pass after
centralizing tests and replacing the production `include!` fragments. Unit
tests were not run. The current full code graph refresh completed with
1,942 nodes and 7,366 edges, with zero skipped files and zero partial parses.
Coverage reports only the ignored `tests/__pycache__` subtree; it reports no
recorded gaps in `src/` or indexed files under `tests/`. These are best-effort
index signals, not proof of completeness. No bundled skill validator was
present in the checkout. The documented module audit script completed. No live OAuth,
Gmail, keyring/OpenBao, deployment, or graphical behavior was verified.

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

Source-level verification does not establish activation, deployment, live
Google consent, keyring/OpenBao availability, browser behavior, or graphical
session health. The current graph snapshot predates no source changes in this
follow-up and recorded zero skipped or partial files; the coverage tool still
provides only a best-effort signal. An earlier snapshot reported a `cli.run`
helper cycle that direct source inspection did not confirm as recursive
production control flow. Documentation mockup images are ignored by design.

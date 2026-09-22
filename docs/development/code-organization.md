# Code organization and Unix-oriented conventions

**Status:** `verified-repository` for the modular source layout and checks in
the current checkout. **Audience:** Arqen contributors and repository-scoped
coding agents.

Arqen uses directory modules as responsibility boundaries. A `mod.rs` file is
a small facade: it owns shared imports/types and composes focused files; it
does not become a second application entrypoint. Public names are re-exported
from the existing facade so callers do not need to know the internal file
layout.

## Dependency direction

Keep dependencies moving toward the process boundary:

```text
models and validation
        ↓
domain/store/service operations
        ↓
protocol and provider adapters
        ↓
process entrypoints, terminal UI, and HTTP routing
```

Models, validation, rendering, and protocol conversion should not open files,
touch the database, launch processes, or call Google. Those effects belong in
the adapter or runtime module that owns the boundary. The TUI renderer may read
state types, but it does not mutate `AccountStore`; command dispatch may wire
services together, but it does not duplicate their rules.

## Unix-oriented rules

1. **Do one thing well.** Give each module one reason to change. Roughly 200
   lines is a recommended review marker, never an absolute ceiling or pass/fail
   gate: split only when the file mixes responsibilities or contains a
   high-cognitive function. Keep a cohesive exception when the audit note
   explains why another split would reduce traceability.
2. **Compose typed stages.** Accept explicit inputs and return domain values or
   `Result<T>`. Prefer `?` and small transformations so one stage's output can
   become the next stage's input without hidden global state.
3. **Stay silent by default.** Do not add debug prints, incidental stdout or
   stderr, secret values, or duplicated status messages. User-visible output
   belongs to the TUI/HTTP contract or the top-level error boundary.
4. **Repair noisily and early.** Validate environment values, paths, request
   sizes, headers, credentials, and state at the boundary. Add operation context
   to errors, fail closed on invalid input, and do not use broad fallbacks to
   hide a broken provider, socket, or database.
5. **Keep effects at the edge.** Filesystem, SQLite, keyring/OpenBao, browser,
   terminal, Unix socket, and HTTP effects should be easy to identify by their
   module path and call site.
6. **Make behavior traceable.** Keep facades explicit, use stable names for
   domain operations, colocate tests with the contract they protect, and update
   the repository map when a responsibility moves.

## Module layout

- `src/config.rs`, `src/cli.rs`, and `src/tui/` own binary startup, terminal
  lifecycle, state transitions, input, OAuth orchestration, and browser work.
- `src/ui/` owns pure Ratatui layout, rendering, modal, and hit-testing code.
- `src/auth/`, `src/gmail/`, `src/broker/`, `src/server/`, and `src/control/`
  separate provider, protocol, and process boundaries.
- `src/store/` owns account models, SQLite schema/migrations, CRUD, and target
  invariants; `src/secrets/` owns credential backends.
- `src/mcp.rs` remains the small broker wire-contract module.

When adding a feature, place the rule in the narrowest owning module, expose it
through the existing facade only when another boundary needs it, and keep
runtime/deployment claims in the relevant documentation rather than comments
that imply live verification.

## Refactoring checklist

- Map callers and consumers before moving a symbol; use the codebase graph when
  available, then verify every important edge in source.
- Preserve public names, wire shapes, schema/migration behavior, secret
  boundaries, keyboard/mouse behavior, and error semantics.
- Move tests with the implementation and retain failure-path coverage.
- Run the focused Cargo checks before expanding to the full repository checks.
- Record remaining graph parse gaps, live/runtime limits, and any intentionally
  cohesive file in `docs/audits/code-modularization.md`.

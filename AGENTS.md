# Repository agent instructions

## Project overview

This repository contains `arqen`, a Rust 2024 terminal UI for
connecting and managing multiple Google accounts. The current implementation
is an account identity and credential-storage slice; it is not coupled to
Hermes or another agent runtime.

### Source map

- `src/main.rs` owns the terminal UI, screen state, account flow, browser
  launching, and clipboard actions.
- `src/auth.rs` owns Google OAuth authorization-code flow with PKCE, callback
  parsing and state validation, profile retrieval, and OS-keyring refresh-token
  storage.
- `src/lib.rs` owns the SQLite account store, schema migration, and account
  metadata CRUD/upsert behavior.
- `src/theme.rs` contains shared Ratatui color tokens. Keep UI changes aligned
  with [`docs/ui-style.md`](docs/ui-style.md).

## Development workflow

Use the repository's Nix environment when available:

```bash
nix develop .#arqen
```

`.envrc` loads the same `arqen` development environment automatically
when Nix is installed. The standard checks are:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build
nix flake check --no-build
```

Run focused checks first, then expand to the affected package or full checks as
needed. `cargo run` starts an interactive terminal application and should not
be used as an automated login test.

### Repository code knowledge MCP

This checkout provides a project-scoped `codebase-memory-mcp` server through
`.mcp.json` and `.codex/config.toml`. When the MCP tools are available (usually
named `mcp__codebase_memory_mcp__...`), use them for non-trivial structural
discovery before broad text search:

- Start with `list_projects` and `index_status` for the current project.
- Use `search_graph`, `trace_path`, `get_architecture`, and `get_code_snippet`
  for symbol, call-path, architecture, and impact questions.
- Use `check_index_coverage` for every cited path and before negative or
  exhaustive claims; inspect flagged source ranges directly.
- Treat direct source as authoritative when graph coverage is partial or
  stale. Use normal file edits, Cargo commands, and tests for implementation and
  verification; the MCP does not replace those tools.

Do not force MCP calls for trivial single-file lookups. If the tools are not
available, continue with direct source inspection and report that the project
MCP was not loaded. Setup and verification details are documented in the
[`README.md`](README.md#repository-code-knowledge-base).

### Editor setup

- Install `rust-lang.rust-analyzer` for Rust completion, diagnostics,
  navigation, refactoring, and Cargo integration.
- Install `jnoortheen.nix-ide` when editing `flake.nix` or other Nix files for
  Nix syntax support and `nixd` integration.
- Open the repository from the Nix development environment, or enable direnv,
  so editor tooling resolves the repository's Rust and Nix tools.

## Data and security boundaries

- The SQLite database is created at
  `$XDG_DATA_HOME/arqen/accounts.sqlite3`, or at
  `~/.local/share/arqen/accounts.sqlite3` when
  `XDG_DATA_HOME` is unset.
- Google OAuth client configuration is read from
  `.secrets/google-client-secret.json` by default, or from the path in
  `GOOGLE_CLIENT_SECRET`.
- SQLite stores account metadata and a keyring reference. OAuth refresh tokens
  are stored in the OS keyring; access tokens exist only during the login
  exchange.
- Never commit, print, copy into documentation, or otherwise expose OAuth
  client JSON, access tokens, refresh tokens, keyring contents, or other
  credentials. Keep secrets and local databases ignored by Git.

Interactive browser redirects, clipboard behavior, OS-keyring integration, and
Google account access are user-owned runtime concerns. Passing tests, building,
or evaluating the Nix flake does not prove those behaviors, activation, or any
live service behavior.

## Change guidelines

- Preserve existing staged, modified, untracked, and ignored-but-relevant user
  files. Do not reset, clean, delete, or overwrite unrelated work.
- Make the smallest coherent change and reuse existing modules, data shapes,
  dependencies, and repository commands. Do not add speculative abstractions
  or dependencies.
- Preserve OAuth state validation, PKCE, credential boundaries, SQLite
  subject-based upsert semantics, and existing error handling unless a change
  explicitly requires otherwise.
- Keep keyboard and mouse interactions consistent with the UI style guide, and
  give new mouse actions a keyboard equivalent.
- Update the nearest relevant README or documentation when behavior or a
  contributor workflow changes. Keep operational, activation, deployment, and
  live-runtime claims clearly separated from source and build evidence.
- Before handoff, inspect the complete diff, run proportionate checks, and
  report passed, failed, not-run, or blocked verification accurately.

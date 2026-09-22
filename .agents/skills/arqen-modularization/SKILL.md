---
name: arqen-modularization
description: Audit or refactor Arqen Rust modules for maintainability, readability, clear purpose, traceability, and the repository's explicit Unix-oriented coding philosophy.
metadata:
  short-description: Modularize Arqen source safely
---

# Arqen modularization

Use this repository-only skill when a change audits or restructures `src/` for
maintainability, readability, clear module purpose, traceability, or
single-responsibility boundaries. Read
[`docs/development/code-organization.md`](../../../docs/development/code-organization.md)
and the current audit note before editing. For the historical and Rust design
references behind the conventions, read
[`references/unix-philosophy.md`](references/unix-philosophy.md). For a quick,
read-only baseline, run
[`scripts/audit-modules.sh`](scripts/audit-modules.sh) from the repository root.

## Concepts this skill enforces

Every audit and refactor must explain how the resulting code improves these
specific qualities:

- **Maintainability:** responsibilities are isolated, files have a clear
  reason to change, and future changes can be made locally.
- **Readability:** names, module paths, control flow, and data transformations
  make intent visible without requiring a reader to reconstruct hidden state.
- **Purpose:** each module, function, and facade has a stated job and does not
  accumulate unrelated policy or side effects.
- **Traceability:** callers, consumers, contracts, tests, errors, and effects
  can be followed from entrypoint to boundary; moved symbols retain stable
  public names or explicit re-exports.

These qualities are implemented through the following Unix philosophy in code:

1. **Doing One Thing Well (Single Responsibility).** Give each module one
   reason to change. Treat roughly 200 lines as a recommended review marker,
   never an absolute ceiling or pass/fail gate; split only when purpose,
   cohesion, or cognitive complexity shows a real maintainability problem.
2. **Expect Output to Become Input (Piping & Composition).** Prefer typed,
   explicit inputs and outputs, `Result<T>` composition, and small stages whose
   output feeds the next stage. Avoid hidden global state and implicit coupling.
3. **The Rule of Silence (No Extraneous Clutter).** Do not add incidental
   stdout/stderr, debug prints, duplicate status messages, or secret values.
   Emit user-facing output only through the owning TUI/HTTP contract or the
   top-level error boundary.
4. **Repair Noisily and Early (Fail-Fast).** Validate configuration, paths,
   request limits, headers, credentials, and state at the boundary. Return
   contextual errors, fail closed, and never use a broad fallback to conceal a
   broken provider, socket, database, or invariant.

## Workflow

1. Snapshot branch/status and read `AGENTS.md`, the docs index, manifests, and
   affected source/tests. Record the inspected revision and exclude live,
   deployment, credential, and activation work.
2. Run `scripts/audit-modules.sh` and measure high-cognitive functions, then
   trace callers and consumers with the repository graph when available. Treat
   direct source as authoritative when graph coverage is partial.
3. Choose responsibility seams using the four Unix principles above, keep
   `mod.rs` facades small, and preserve
   existing public names, CLI commands, environment variables, schemas, wire
   formats, credential boundaries, and UI interactions. Do not add a dependency
   merely to split a file.
4. Isolate side effects at process, filesystem, database, provider, terminal,
   Unix-socket, and HTTP boundaries. Record each boundary's purpose and
   failure path. Compose typed inputs/outputs, fail fast with contextual
   errors, and keep internal code silent.
5. Move tests with their implementation and retain success and failure-path
   coverage. Update the repository map and canonical code-organization note
   only for claims established by source or checks.
6. Run focused checks first, then `cargo fmt --check`, `cargo test`, clippy with
   warnings denied, `cargo build`, `nix flake check --no-build`, and
   `git diff --check`. Validate the script, references, and this skill with the
   bundled skill validator.

## Completion report

Report changed module boundaries and, for each meaningful seam, the
maintainability, readability, purpose, and traceability improvement. Explicitly
state how the refactor applies Doing One Thing Well, Expect Output to Become
Input, the Rule of Silence, and Repair Noisily and Early. Also report preserved
public contracts, checks and exact status, graph coverage gaps, intentionally
cohesive files, and runtime/live verification that was not performed. Do not
claim that a source/build check proves deployment, activation, OAuth, Gmail,
keyring, or graphical behavior.

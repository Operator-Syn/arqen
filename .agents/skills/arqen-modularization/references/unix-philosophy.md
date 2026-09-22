# Arqen modularization reading references

Read this note when deciding whether a module should be split, how a boundary
should compose, or how a failure/output contract should behave. These are
source references and interpretation aids; they do not override `AGENTS.md`,
the repository code, or an explicit user requirement.

## Historical Unix sources

- [Bell Labs UNIX history: programming philosophy](https://www.nokia.com/bell-labs/unix-history/philosophy.html) — archival context for the toolbox, pipes, small cooperating programs, and text-stream interfaces.
- [The Unix Philosophy](https://unixphilosophy.com/) — compact transcription and explanation of the 1978 McIlroy, Pinson, and Tague principles: do one thing well, compose outputs and inputs, avoid extraneous output, and try software early.
- [The Art of Unix Programming](https://www.catb.org/~esr/writings/taoup/html/) — Eric S. Raymond's extended treatment of the Rule of Silence, Rule of Repair, least surprise, interfaces, and modular composition.

Use the historical sources to explain *why* a boundary is being made small or
composable. Do not turn a slogan into a blind file-size rule. The script's
roughly 200-line value is a recommendation for review, not an absolute ceiling
or pass/fail gate; a cohesive renderer or test fixture can remain larger when
splitting it would hide the contract.

## Rust boundary references

- [Rust Book: Recoverable Errors with `Result`](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) — returning recoverable failures to the caller and composing them with `?`.
- [Rust Book: To `panic!` or Not to `panic!`](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html) — choosing a recoverable boundary versus an unrecoverable invariant failure.
- [Rust API Guidelines: meaningful error types](https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err) — public errors should implement `Error`, `Display`, and carry useful context.
- [Rust API Guidelines: hidden implementation details](https://rust-lang.github.io/api-guidelines/documentation.html#c-hidden) — keep internal implementation details out of stable public contracts and use `pub(crate)` where appropriate.

Use these references when a split changes a public facade, an error type, a
`Result` pipeline, or the point at which invalid input is rejected.

## Arqen application

Map the reading to the repository as follows:

| Principle | Arqen evidence to inspect |
| --- | --- |
| Do one thing well | `src/tui/`, `src/ui/`, and service directory facades; each module's reason to change |
| Output becomes input | typed service/protocol functions, `Result<T>` stages, broker framing, and MCP request/response boundaries |
| Rule of Silence | TUI/HTTP-owned user messages, no incidental prints, and tests that prevent credential leakage |
| Repair noisily and early | configuration/request validation, contextual `anyhow` errors, fail-closed auth, and boundary tests |

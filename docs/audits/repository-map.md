# Repository map

**Reviewed:** 2026-09-11. **Confidence:** `verified-repository` unless noted.

| Path | Role | Key contract |
| --- | --- | --- |
| `src/main.rs` | TUI state, commands, OAuth flow, CLI dispatch | `t` persists target; remote callback mode; `credential-broker` and `mcp-server` entry points |
| `src/auth.rs` | Google PKCE/token/keyring operations | exact returned scopes; refresh tokens stay in keyring |
| `src/lib.rs` | SQLite schema and CRUD | `mcp_configuration` singleton with target subject |
| `src/ui/accounts.rs` | Account cards and details panes | target marker, scope presentation, scrollbars |
| `src/ui/chrome.rs` | Header/footer actions and notices | focus/wheel/target affordances |
| `src/ui/dialogs.rs` | Authorization, redirect, and error surfaces | modal copy and responsive dialog layout |
| `src/ui/modal.rs` | Reusable modal geometry and hit testing | action IDs without side effects |
| `src/ui/theme.rs` | Shared Ratatui color tokens | consistent status/focus colors |
| `src/ui/mod.rs` | Layout, pane focus, and mouse routing | responsive geometry and scroll state |
| `src/callback.rs` | Loopback OAuth listener/helper | callback and optional launcher routes |
| `src/gmail.rs` | Gmail REST client | bounded list + metadata get |
| `src/broker.rs` | Unix credential broker/client | eligibility, refresh, stable errors |
| `src/mcp.rs` | Broker wire types | operation/result/error serialization |
| `src/server.rs` | Streamable HTTP MCP process | bearer/Host/Origin gate and `list_emails` |
| `Dockerfile` | MCP container image | non-root runtime |
| `deploy/` | Docker MCP first-pass, systemd alternatives, Nginx examples | operator-owned deployment boundary |
| `.env.example` | local configuration template | safe defaults; no secrets |
| `Makefile` / `scripts/` | named local workflows and smoke checks | sources ignored `.env`; explicit live-call opt-in |
| `scripts/arqen-quickstart.sh` | user binary/config/unit preparation | no activation unless explicitly requested |
| `flake.nix` | Pinned development shell | Rust/tooling entry point |
| `Cargo.toml` / `Cargo.lock` | Package and dependency contract | reproducible Rust dependency resolution |
| `docs/` | architecture and evidence map | no secrets or live credentials |

The root README remains the quick-start surface; this tree provides the
deeper architecture, API, security, operations, decisions, and verification
context.

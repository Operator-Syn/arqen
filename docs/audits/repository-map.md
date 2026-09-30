# Repository map

**Reviewed:** 2026-10-01. **Confidence:** `verified-repository` unless noted.

| Path | Role | Key contract |
| --- | --- | --- |
| `src/main.rs`, `src/cli.rs`, `src/config.rs` | Thin binary entrypoint, command dispatch, XDG/OAuth configuration | unchanged CLI commands and environment defaults |
| `src/tui/` | TUI state, account/login transitions, browser lifecycle, input, runtime | `t` persists target; keyboard/mouse behavior and OAuth flow preserved |
| `src/control/` / `src/control/login.html` | Branded local control gateway and ttyd proxy | password/session gate, themed login page, HTTP/WebSocket forwarding |
| `src/auth/` | Google PKCE/token/protected-store operations | exact returned scopes; refresh tokens stay outside SQLite and MCP |
| `src/store/` / `src/lib.rs` | SQLite models, schema/migrations, CRUD, and root re-exports | `mcp_configuration` singleton with target subject |
| `src/ui/accounts/` | Account cards, details, status, scrolling, and hit testing | target marker, scope presentation, scrollbars |
| `src/ui/chrome/` | Header/footer actions, notices, and affordances | focus/wheel/target behavior |
| `src/ui/dialogs/` | Authorization, redirect, error, and confirmation surfaces | modal copy and responsive dialog layout |
| `src/ui/modal/` | Reusable modal geometry, rendering, and hit testing | action IDs without side effects |
| `src/ui/theme.rs` | Shared Ratatui color tokens | consistent status/focus colors |
| `src/ui/` | Layout, rendering composition, pane focus, and mouse routing | responsive geometry and scroll state |
| `src/callback/` | Loopback OAuth listener/helper, request parsing, and pages | callback and optional launcher routes |
| `src/gmail/` | Gmail REST models, validation, client, response mapping, and separate draft provider adapter | labels, bounded message/draft metadata, read-state, message Trash, draft create/reply/list/delete/send |
| `src/broker/` | Unix credential broker/client, framing, focused email/draft/label/action/target/token/error modules | selected target, eligibility, mutually exclusive one-use account-bound action marks, refresh, stable errors |
| `src/mcp.rs` | Broker wire types | label, message, draft, action-mark, and readiness operations with result/error serialization |
| `src/server/` | Streamable HTTP MCP process, auth, routes, and runtime | bearer/Host/Origin gate and Gmail message, label, read-state, Trash, and guarded draft tools |
| `tests/` | Central test root: Rust unit modules mirror `src/`, Python suites, future integration suites | `tests/unit/`, `tests/python/`, and `tests/integration/` provide separate discovery and ownership scopes |
| `Dockerfile` | Docker-native MCP (`mcp` target) and control/broker (`runtime` target) images | non-root runtimes; control command is the gateway-owned TUI process |
| `compose.yaml` | User-facing stack using published GHCR images | no source build; loopback host ports; persistent named volumes |
| `deploy/` | Docker-native OpenBao stack, systemd alternatives, Nginx example | operator-owned deployment boundary |
| `.env.example` | local configuration template | safe defaults; no secrets |
| `Makefile` / `scripts/` | named local workflows and smoke checks | sources ignored `.env`; explicit live-call opt-in |
| `scripts/arqen-quickstart.sh` | user binary/config/unit preparation | no activation unless explicitly requested |
| `flake.nix` | Pinned development shell | Rust/tooling entry point |
| `Cargo.toml` / `Cargo.lock` | Package and dependency contract | reproducible Rust dependency resolution |
| `docs/` | architecture and evidence map | no secrets or live credentials |

The root README points users to `docs/quickstart.md`; this tree provides the
deeper architecture, API, security, operations, decisions, and verification
context. Make-based source-build workflows remain documented for developers.

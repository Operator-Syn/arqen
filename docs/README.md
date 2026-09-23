# Arqen documentation

This directory is the source-grounded map for the Arqen account TUI and the
single-target Gmail MCP slice. It describes the repository as implemented at
the current revision; it does not claim that a broker, container, TLS proxy,
or Google account has been deployed or exercised live.

**Verification state:** `verified-repository` for the Rust code, tests, and
checked-in examples; `verified-external` for the linked MCP and Gmail API
contracts; `deployment-ready example` for the Docker-native, legacy Compose,
systemd, and Nginx templates. User-service activation, keyring/OpenBao
persistence, OAuth, and public deployment remain operator-owned. Last
reviewed: 2026-09-22.

## Document tree

```text
docs/
├── README.md
├── api/
│   ├── gmail.md
│   └── mcp.md
├── architecture/
│   ├── overview.md
│   └── runtime-topology.md
├── audits/
│   ├── code-modularization.md
│   ├── documentation-gaps.md
│   └── repository-map.md
├── components/
│   ├── gmail-broker.md
│   ├── mcp-server.md
│   ├── oauth-and-keyring.md
│   └── tui.md
├── data/
│   └── sqlite.md
├── decisions/
│   ├── ADR-001-target-account.md
│   ├── ADR-002-host-credential-broker.md
│   ├── ADR-003-bearer-v1-and-oauth-later.md
│   ├── ADR-004-always-on-user-services.md
│   └── ADR-005-docker-mcp-first-pass.md
├── development/
│   ├── code-organization.md
│   ├── local-workflows.md
│   └── verification.md
├── operations/
│   ├── container-and-nginx.md
│   └── credential-broker.md
├── security/
│   └── auth-and-data.md
├── ui-mockups/
│   ├── README.md
│   ├── arqen-confirm-quit-modal.png
│   └── google-account-tui-dashboard.png
└── ui-style.md
```

## Start here

- [Architecture overview](architecture/overview.md) explains ownership and
  data flow.
- [MCP API](api/mcp.md) is the client-facing Streamable HTTP contract.
- [Gmail broker](components/gmail-broker.md) explains why refresh tokens stay
  outside the MCP HTTP process in both native and Docker-native modes.
- [Target-account decision](decisions/ADR-001-target-account.md) records the
  one-account-at-a-time policy.
- [Verification](development/verification.md) separates source/build proof
  from deployment and live Google behavior.
- [Code organization](development/code-organization.md) defines module
  boundaries and Unix-oriented maintainability conventions.
- [Code modularization audit](audits/code-modularization.md) records the
  baseline hotspots and resulting source boundaries.
- [Local workflows](development/local-workflows.md) documents `.env`, named
  scripts, native/container smoke checks, and the repository code-knowledge MCP
  bootstrap prompt.

The existing [UI style guide](ui-style.md) and [TUI mockup notes](ui-mockups/README.md)
remain the visual source of truth for the account dashboard. The root
[README](../README.md) is the user-facing quick start.

The repository-local [`arqen-modularization` skill](../.agents/skills/arqen-modularization/SKILL.md)
is the reusable agent workflow for future source-boundary audits and refactors.

## Feature and responsibility index

- **Account identity:** multiple Google identities, subject-based upsert,
  persisted connection state, and exact last-granted scopes.
- **OAuth lifecycle:** PKCE authorization, loopback callback handling, owned
  browser cleanup, reconnect/reauthentication, and reusable error/confirmation
  modals.
- **Responsive TUI:** focusable account/details panes, keyboard and wheel
  scrolling, persistent visual scrollbar gutters, target/status markers, and
  compact-layout footer affordances.
- **Control sign-in:** loopback gateway with shared TUI tokens, generated
  password validation, memory-only sessions, and ttyd HTTP/WebSocket proxying.
- **Target configuration:** one persisted MCP target, selected explicitly in
  the TUI, with eligibility checks and fail-closed stale-target handling.
- **Credential boundary:** OpenBao-backed Docker broker or native keyring broker
  with in-memory access-token caching; no token values cross the broker protocol
  or enter SQLite.
- **Gmail listing:** bounded inbox/search pagination, metadata-only headers,
  labels, and Unicode-safe snippet truncation.
- **MCP transport:** Streamable HTTP `/mcp`, authenticated health probe,
  readiness probe, bearer gate, Host/Origin allowlists, and one advertised
  read-only tool.
- **Operations:** native loopback services plus a Docker-native OpenBao/broker/
  streamed-TUI/MCP profile for clean-slate local deployment, with the legacy
  VPS and Nginx examples retained as deferred alternatives; activation, TLS,
  and public deployment remain operator-owned.
- **Local workflows:** `.env.example`, Make targets, Nix-aware Cargo wrappers,
  remote SSH OAuth instructions, quickstart installation, and disposable
  native/Compose smoke paths.

Notable constraints are recorded in the ADRs: target choice is not inferred
from the selected TUI row, remote callers cannot choose an account, Gmail’s
read-only scope is restricted, and disconnected/revoked credentials never
trigger an automatic SQLite fallback or mutation.

# ADR-002: Keep keyring access in a host broker

- **Status:** Accepted for native and legacy VPS compatibility paths; the
  Docker-native local profile uses the same broker boundary with OpenBao.
- **Decision:** `arqen credential-broker` runs on the TUI host over a
  user-only Unix socket. `arqen mcp-server` (including its container) sends a
  bounded request and receives summaries/errors only.
- **Reason:** Containers and public HTTP handlers should not receive refresh
  tokens or keyring access. The host broker also reuses the existing SQLite
  account store and keyring coordinates.
- **Trade-off:** The broker lifecycle and socket mount must be supervised by
  the operator; no automatic service activation is included.

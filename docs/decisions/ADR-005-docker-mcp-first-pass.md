# ADR-005: Docker MCP first-pass VPS deployment

- **Status:** Accepted for the first-pass VPS build.
- **Decision:** Run the TUI, SQLite account store, OS keyring, and credential
  broker on the VPS host. Run only `mcp-server` in Docker Compose with a
  read-only broker-socket mount and `restart: unless-stopped`.
- **Reason:** The Docker container is the headless network boundary, while the
  host remains the credential boundary. This avoids moving the OS keyring or
  OAuth client JSON into a container and keeps the TUI usable over SSH.
- **OAuth decision:** The TUI process runs on the VPS and is rendered in the
  operator’s local terminal. A local browser reaches its loopback callback
  through an SSH `-L` tunnel; Docker is not involved in the OAuth callback.
- **Operational invariant:** The Docker MCP container must never become an
  independent account/configuration store. It returns broker readiness and
  target errors until the host TUI has selected an eligible account.
- **Deferred boundary:** v1 has no local TUI-to-remote-backend control
  protocol. The TUI itself runs on the VPS over SSH; any future split TUI would
  require a separately designed and authenticated control API.
- **Alternative:** The checked-in native `arqen-mcp.service` remains available
  for hosts that do not use Docker, but it is not enabled by the first-pass
  VPS quickstart.

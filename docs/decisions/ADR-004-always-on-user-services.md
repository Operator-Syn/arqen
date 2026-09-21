# ADR-004: Unattended user services and SSH loopback OAuth

- **Status:** Accepted as the native-host alternative; Docker-native local
  deployment is documented separately and the legacy Docker VPS deployment is
  recorded in ADR-005.
- **Decision:** Run `credential-broker` and `mcp-server` as separate systemd
  user services under the same Linux user as the TUI and OS keyring. Prepare
  them with `make quickstart`; activation and user lingering are explicit
  operator actions.
- **Reason:** The TUI is a configuration frontend, while the broker and MCP
  endpoint must remain available after the TUI and SSH session exit.
- **OAuth decision:** Headless VPS login uses a fixed loopback callback port
  forwarded with one SSH local-forward command. The callback is never exposed
  publicly and Google’s deprecated OOB flow is not used.
- **Readiness invariant:** HTTP liveness is separate from broker readiness. A
  missing target, unavailable keyring, or stopped broker keeps the service
  process recoverable but makes `/readyz` fail closed.
- **Session invariant:** MCP sessions remain process-local and clients
  reinitialize after a restart; account and target configuration remain in
  SQLite.
- **First-pass relationship:** ADR-005 selects the host broker plus Docker
  MCP combination for the initial VPS build; this ADR remains the all-native
  fallback.

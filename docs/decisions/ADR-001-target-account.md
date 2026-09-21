# ADR-001: Explicit single MCP target

- **Status:** Accepted for the first Gmail tool.
- **Decision:** Persist one `google_subject` in the `mcp_configuration`
  singleton. The TUI’s `t` action sets, replaces, or clears it.
- **Reason:** A private operator can see exactly which identity a remote tool
  call will use; provider discovery and per-request account choice are later
  work.
- **Invariant:** Only connected accounts with confirmed Gmail read-only scope
  evidence and a protected credential reference can be selected. A stale target remains
  visible but is never used as an automatic fallback.

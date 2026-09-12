# ADR-003: Bearer token for v1, MCP OAuth later

- **Status:** Accepted for the private single-operator milestone.
- **Decision:** Require one rotated bearer token from an environment variable
  or secret file, plus Host/Origin allowlists. Do not implement MCP-native
  OAuth authorization yet.
- **Reason:** It keeps the first remote surface small while preserving the
  official Streamable HTTP transport and a clear upgrade boundary.
- **Follow-up:** A future OAuth design must define resource metadata,
  authorization-server discovery, token audience/issuer validation, rotation,
  and multi-user target semantics before replacing this gate.


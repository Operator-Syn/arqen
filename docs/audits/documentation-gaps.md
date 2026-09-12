# Documentation gaps and open work

These items are intentionally outside the current implementation:

- live Google OAuth/Gmail verification and account-specific consent checks;
- Google restricted-scope verification, privacy-policy review, and production
  user-data approval;
- TLS certificate issuance, DNS/firewall configuration, and public deployment;
- secret generation/rotation and container UID/GID coordination;
- MCP-native OAuth authorization and multi-user target selection;
- additional Gmail tools such as full-body search, attachments, send, or
  mutation operations;
- broker socket activation and platform support outside Unix hosts.

The checked-in examples document the expected boundaries and placeholders;
they do not imply that any of these items has been completed.


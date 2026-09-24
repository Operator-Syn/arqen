# Documentation

Use this map to find the right guide for Arqen's account manager, Gmail MCP
service, development workflow, and local operations. These pages describe
checked-in code and examples; they do not claim that services have been
activated or deployed.

> **Operational boundary:** OAuth, keyring/OpenBao persistence, service
> activation, TLS, and public exposure require operator action and separate
> live verification.

## Start with what you need

| Goal | Guide |
| --- | --- |
| Get oriented and start Arqen | [Project README](../README.md) |
| Choose native or Docker-native setup | [Local workflows](development/local-workflows.md) |
| Understand process ownership and data flow | [Architecture overview](architecture/overview.md) · [Runtime topology](architecture/runtime-topology.md) |
| Connect an MCP client | [MCP API](api/mcp.md) · [MCP server](components/mcp-server.md) |
| Review account and credential handling | [Authentication and data](security/auth-and-data.md) · [Credential broker](operations/credential-broker.md) |
| Run containers or configure an Nginx edge | [Container operations](operations/container-and-nginx.md) |
| Pull images or understand releases | [Docker image releases](operations/docker-images.md) |
| Build and verify a change | [Verification](development/verification.md) · [Code organization](development/code-organization.md) |
| Browse design decisions and audits | [Decisions](decisions/) · [Audits](audits/) |

## Product boundaries

- The Docker-native Compose profile is the maintained container workflow. The
  former host-broker-plus-Docker-MCP deployment is retired; its ADR remains as
  historical context.
- Native local operation and SSH-forwarded remote OAuth login remain supported.
- MCP requests use the account explicitly selected in the TUI. Callers cannot
  choose an account or access credential values.
- Repository checks establish source/build facts, not OAuth success, service
  activation, container runtime health, or public deployment.

The [UI style guide](ui-style.md) and [mockup notes](ui-mockups/README.md)
cover the account dashboard. The repository-local
[`arqen-modularization` workflow](../.agents/skills/arqen-modularization/SKILL.md)
guides future source-boundary reviews.

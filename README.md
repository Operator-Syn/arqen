# Arqen

**A Rust terminal app for managing Google accounts and choosing the account a
local agent can use.** Arqen exposes a bounded Gmail MCP service and keeps
credential access behind a separate broker.

## Choose a workflow

| Workflow | Best for | Start here |
| --- | --- | --- |
| Docker-native local stack | A clean-slate profile with the TUI, OpenBao, broker, and MCP service in Compose | [Docker setup](docs/operations/container-and-nginx.md) |
| Native local | Using the OS keyring and running the broker/MCP processes on the host | [Local workflows](docs/development/local-workflows.md) |
| Headless login over SSH | Completing OAuth for a TUI running on a remote host | [SSH OAuth instructions](docs/development/local-workflows.md#remote-oauth-over-ssh) |

The retired host-broker-plus-Docker-MCP Compose deployment is no longer
maintained. SSH forwarding remains supported for the OAuth callback; it is not
a Docker deployment mode.

## Quick start: Docker-native

On a graphical host with Docker Compose and a running Docker daemon:

```bash
make docker-setup   # first run: create the local profile and protected files
make docker-up
```

Open `http://127.0.0.1:7681`, sign in as `arqen` with the generated password in
`.secrets/arqen-control-password`, complete Google login, and press `t` on the
account to select it as the MCP target. The MCP endpoint is
`http://127.0.0.1:8787/mcp`; connect an MCP client with the bearer token stored
in `.secrets/mcp-bearer-token`.

By default, the stack builds from the current checkout. To pull a published
GHCR release, set `ARQEN_DOCKER_IMAGE_SOURCE=registry` in the ignored `.env`;
set `ARQEN_DOCKER_IMAGE_TAG` to select a version (default: `latest`). Images
support `linux/amd64` and `linux/arm64`; Docker selects the matching platform.
See [image releases and pulls](docs/operations/docker-images.md).

Use `make docker-status` to inspect local readiness and `make docker-down` to
stop the stack while keeping its data. The explicit
`make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES` command deletes the profile
and its generated OpenBao/control secrets.

## Native local setup

Create the ignored local configuration and generated bearer token, then start
the TUI:

```bash
make setup-local
make tui
```

Add the Google desktop-client JSON at `.secrets/google-client-secret.json`
before starting login. After selecting an account with `t`, run `make backend`
to supervise the native credential broker and loopback MCP server. Setup copies
`.env.example` only when `.env` is absent; it does not overwrite or synchronize
an existing `.env`.

## What Arqen provides

- Multiple Google account identities in a migrated SQLite metadata store.
- OAuth login, reconnect, reauthentication, and confirmed disconnect/revocation.
- One explicitly selected MCP account target; callers cannot choose an account.
- Gmail list/read, label, and read-state MCP tools with bounded requests.
- Separate HTTP and credential-broker processes. Native mode uses the OS
  keyring; the Docker profile uses OpenBao.

The local MCP service binds to loopback by default. `/healthz` reports HTTP
liveness; `/readyz` checks local broker, database, target, and credential
readiness without contacting Gmail. A successful tool call is the first live
Gmail check. Public exposure requires an operator-managed TLS proxy and review
of Google's current user-data requirements.

## Documentation

| Need | Guide |
| --- | --- |
| Find the right guide | [Documentation map](docs/README.md) |
| Understand runtime boundaries | [Architecture overview](docs/architecture/overview.md) |
| Configure and run local workflows | [Local workflows](docs/development/local-workflows.md) |
| Operate containers or an Nginx edge | [Docker and reverse proxy](docs/operations/container-and-nginx.md) |
| Pull or publish images | [Docker image releases](docs/operations/docker-images.md) |
| Review MCP request/response behavior | [MCP API](docs/api/mcp.md) |
| Build and verify changes | [Development verification](docs/development/verification.md) |

## Development

Use the repository's Rust environment, then run the standard checks:

```bash
nix develop .#arqen
make check
```

The repository contains workflow templates and local examples. Checks do not
activate services, perform OAuth, contact Gmail unless explicitly requested,
issue certificates, or deploy a public endpoint.

## License

Arqen is distributed under the **Mozilla Public License 2.0 (MPL-2.0)**. See
[`LICENSE`](LICENSE) for the complete terms. MPL-covered source files retain
their MPL terms when modified; separate files in a larger work may use other
licenses where their terms allow it.

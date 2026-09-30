# Docker Compose and remote access

## User Compose stack

The root [`compose.yaml`](../../compose.yaml) pulls the published GHCR images
and runs Arqen without a source build. Follow the [Quickstart](../quickstart.md)
to add your Google OAuth client, start the services, select an account, and
connect an MCP client.

The control page, OAuth callback, and MCP endpoint bind to localhost. OpenBao
is internal to the Compose network. `docker compose down` stops the services
while keeping their named data volumes.

## Source-build desktop stack

The Compose files under [`deploy/containers/`](../../deploy/containers/) are
used by developer Make commands. `make docker-up` builds Arqen from the current
checkout by default and selects a Wayland or X11 display override for the
control terminal. See [development workflows](../development/local-workflows.md)
for setup, image modes, and lifecycle commands.

## Remote MCP access

The user stack publishes MCP on loopback port 8787. To connect from another
machine, put a TLS reverse proxy on the Docker host. The
[`deploy/nginx/arqen-mcp.conf`](../../deploy/nginx/arqen-mcp.conf) file is an
example. Replace its domain and certificate paths, set matching
`ARQEN_MCP_ALLOWED_HOSTS` and `ARQEN_MCP_ALLOWED_ORIGINS`, and review its rate,
body-size, and timeout settings. `proxy_buffering off` and HTTP/1.1 preserve
request-scoped streaming.

The control page and OAuth callback remain local. For a Docker host you access
over SSH, forward ports 7681 and 8765 to your computer; do not expose the OAuth
callback to the public internet. The configuration examples do not create
certificates, DNS records, firewall rules, or public deployments.

# Container and reverse proxy examples

`Dockerfile` builds the same Rust binary and starts `arqen mcp-server` as a
non-root user. `deploy/containers/docker-compose.yml` mounts the host broker
socket read-only, reads the bearer token from a secret file, drops Linux
capabilities, and publishes only a configurable loopback port (8787 by
default) for Nginx.

The local scripts make the example easier to exercise without a second env
export:

```bash
make setup-local
make broker          # terminal A
make compose-up      # terminal B
make compose-down
```

For the first-pass VPS deployment, run `make vps-up`. It keeps the TUI,
SQLite, OS keyring, and credential broker on the VPS host while Docker runs
only the always-on MCP HTTP boundary. The container receives a read-only broker
socket mount and never receives refresh tokens or the OAuth client JSON.

`make compose-smoke` creates its own project name, token, runtime directory,
and host port, then removes them on exit. It verifies the authenticated health
and tool-discovery paths without contacting Google. Use
`make compose-smoke-call` only when a live Gmail request is intended. The
broker must still run on the host under the same user as the TUI.

`deploy/nginx/arqen-mcp.conf` is an example public TLS boundary. Before use,
replace the placeholder host/certificate paths, set matching
`ARQEN_MCP_ALLOWED_HOSTS` and `ARQEN_MCP_ALLOWED_ORIGINS`, and review the
rate/body/time-out values. `proxy_buffering off` and HTTP/1.1 preserve
request-scoped streaming behavior.

These files are declarative examples only. No `docker build`, container start,
certificate issuance, DNS change, firewall change, or remote deployment is
performed by repository tests.

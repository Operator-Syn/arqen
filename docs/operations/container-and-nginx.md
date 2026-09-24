# Container and reverse proxy examples

`Dockerfile` builds the same Rust binary and starts `arqen mcp-server` as a
non-root user. `deploy/containers/docker-compose.yml` mounts the host broker
socket read-only, reads the bearer token from a secret file, drops Linux
capabilities, and publishes only a configurable loopback port (8787 by
default) for Nginx. The Compose build uses host networking only for package/DNS
resolution while constructing the image; the running MCP service stays on the
normal Compose network.

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

## Docker-native local profile

The separate `deploy/containers/docker-native-compose.yml` profile is the
primary same-computer deployment. `make docker-setup` initializes a fresh
OpenBao volume and generated Docker secrets; `make docker-up` then starts the
OpenBao-backed broker, control gateway/TUI, and MCP service. It publishes only
loopback ports 7681 (control gateway), 8765 (OAuth callback), and 8787 (MCP);
ttyd stays on control-container loopback port 7682. OpenBao is not published,
and the MCP container receives only its bearer token and broker socket. Run
`make docker-up` from a Wayland or X11 graphical session: only the control
container receives the detected native clipboard interface. The control
container is session-owned and must not be restored by the system Docker daemon
before the graphical socket exists. Use the installed
`arqen-docker-control.service` user unit for session-bound automatic startup.
Use `make docker-down` to stop without deleting volumes or the explicit
`make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES` path to remove the fresh
profile. This path is clean-slate only; it does not migrate native keyring
accounts.

`make docker-up` builds from source unless `ARQEN_DOCKER_IMAGE_SOURCE` is set
to `registry` or `bundle`. Registry mode pulls the configured stable GHCR tags
then starts without building. Bundle mode loads the archive produced by
`make docker-bundle` and also starts without building. The ignored AMD64 bundle
contains OCI layouts, application and base images, and a service-to-image
manifest; it excludes runtime-mounted `.secrets`. See
[`docker-images.md`](docker-images.md) for release tags, version automation,
and package visibility setup.

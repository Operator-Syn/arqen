# Docker-native containers and reverse proxy

The Docker-native local profile is the supported container deployment.
`make docker-setup` initializes a fresh OpenBao volume and generated Docker
secrets; `make docker-up` starts the OpenBao-backed broker, control gateway/TUI,
and MCP service. It publishes only loopback ports 7681 (control gateway), 8765
(OAuth callback), and 8787 (MCP); ttyd stays on control-container loopback port
7682. OpenBao is not published, and the MCP container receives only its
bearer-token file and broker socket.

Run `make docker-up` from a Wayland or X11 graphical session. Only the control
container receives the detected native clipboard interface and browser session
access. The control container is session-owned and must not be restored by the
system Docker daemon before the graphical socket exists. Use the installed
`arqen-docker-control.service` user unit for session-bound automatic startup.
Use `make docker-down` to stop without deleting volumes. The explicit
`make docker-reset ARQEN_DOCKER_RESET_CONFIRM=YES` path removes the fresh
profile and generated OpenBao/control secrets. This path is clean-slate only;
it does not migrate native keyring accounts.

`make docker-up` builds from source unless `ARQEN_DOCKER_IMAGE_SOURCE` is set
to `registry` or `bundle`. Registry mode pulls the configured stable GHCR tags
then starts without building. Bundle mode loads the archive produced by
`make docker-bundle` and also starts without building. The ignored AMD64 bundle
contains OCI layouts, application and base images, and a service-to-image
manifest; it excludes runtime-mounted `.secrets`. See
[`docker-images.md`](docker-images.md) for release tags, version automation,
and package visibility setup.

## Nginx reverse proxy

The Docker-native stack binds the MCP service to loopback port 8787. The
[`deploy/nginx/arqen-mcp.conf`](../../deploy/nginx/arqen-mcp.conf) file is an
example public TLS boundary for that service. Before use, replace placeholder
host and certificate paths, set matching `ARQEN_MCP_ALLOWED_HOSTS` and
`ARQEN_MCP_ALLOWED_ORIGINS`, and review the rate, body, and timeout values.
`proxy_buffering off` and HTTP/1.1 preserve request-scoped streaming behavior.

These files are declarative examples only. No container start, certificate
issuance, DNS change, firewall change, or remote deployment is performed by
repository checks.

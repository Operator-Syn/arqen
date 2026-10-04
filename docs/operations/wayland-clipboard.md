# Wayland Docker stack operations

Use `compose-wayland.sh` for the published-image stack with desktop clipboard
access. The standard Quickstart is unchanged; this guide covers the optional
Wayland setup and its day-to-day operation.

## Update to the latest release

From your deployment directory, run these commands as your normal desktop user
in a logged-in Wayland session:

```bash
./compose-wayland.sh pull
./compose-wayland.sh up -d
./compose-wayland.sh ps
```

No image-tag export is needed for normal updates. The Compose files default to
`latest` when `ARQEN_DOCKER_IMAGE_TAG` is unset or empty. Successful image releases
update that tag; `latest` is a moving reference, not a permanent version pin.

- `pull` downloads the selected published images without restarting services.
- `up -d` applies the images and starts services in the background. The wrapper
  first stops control, broker, and MCP writers before aligning volume ownership.
  Expect a short interruption; it does not build images or erase named volumes.
- `ps` reports container status. Successful one-shot helpers may have exited;
  use `./compose-wayland.sh ps -a` to include them.

Use the wrapper for both pull and startup so the Wayland overlay and desktop
variables remain consistent. A plain `docker compose up` can omit the overlay.

**Privacy warning:** successful `up -d` prints the control password and MCP bearer
token. Run it in a private terminal; do not paste its output into chat, tickets,
logs, or screenshots. If startup reports a failure, investigate it before any
Gmail operation. An image update alone does not verify Gmail behavior.

## Files and prerequisites

Keep these files together in the deployment directory:

- `compose.yaml`: published-image services, network ports, secrets, and volumes.
- `compose.wayland.yaml`: desktop identities, clipboard socket mount, and
  one-shot data/runtime volume ownership helper.
- `compose-wayland.sh`: supplies the desktop environment and invokes both files.
- `.secrets/google-client-secret.json`: private Google OAuth client configuration
  referenced by the base Compose file. Never commit or share this file.
- Optional `.env`: Compose settings such as an image tag or host ports.

Docker and Docker Compose v2 must be available to your desktop user. Run the
wrapper without `sudo`; otherwise UID/GID and desktop socket discovery can refer
to the wrong user. Do not switch Compose project names during a routine update.
Existing volumes are associated with the deployment's Compose project.

The wrapper accepts an absolute `WAYLAND_DISPLAY` socket or resolves a relative
name under `XDG_RUNTIME_DIR`. When `WAYLAND_DISPLAY` is unset, it tries
`${XDG_RUNTIME_DIR:-/run/user/<desktop-uid>}/wayland-0`. Startup requires the
resolved socket to exist.

## Start and use the stack

For an already configured deployment:

```bash
./compose-wayland.sh up -d
./compose-wayland.sh ps
```

Default endpoints are:

| Purpose | Endpoint |
| --- | --- |
| Control UI | `http://127.0.0.1:7681` |
| OAuth callback | `http://127.0.0.1:8765/oauth2/callback` |
| MCP | `http://127.0.0.1:8787/mcp` |

The base Compose file supports host-port overrides; the launcher prints the
actual control/MCP addresses. The control username is `arqen`. On first use,
sign in to the control UI, connect your Google account, and select the MCP target
in Arqen. At the Google authorization prompt, `c` copies through the desktop
clipboard. Routine image updates should not require reconnecting accounts.

Only `arqen-control` receives the host Wayland socket. App services use the desktop
UID/GID to share the private broker socket. The ownership helper changes ownership
of existing data/runtime volumes; it does not clear their contents. The base
Compose file remains responsible for its OAuth secret mounts or staging services;
do not replace it with an unrelated stack's configuration.

## Latest versus a pinned release

The image expression is `${ARQEN_DOCKER_IMAGE_TAG:-latest}`. A shell value takes
precedence over a value in Compose's `.env`. This variable selects the image tag;
it does not enable updating or change how data is retained.

To follow the latest release, leave the variable unset. If you previously exported
a version in the current shell, clear it:

```bash
unset ARQEN_DOCKER_IMAGE_TAG
```

Also remove a version pin from `.env`, if present. Then use the normal update
commands above. If you deliberately want a reproducible version, set an existing
published tag in `.env`, for example:

```dotenv
ARQEN_DOCKER_IMAGE_TAG=0.1.9
```

That pin selects the same release for the Arqen runtime, MCP, and project OpenBao
images. It will not advance to later releases until you change/remove it. You can
also export the variable for a temporary shell-only pin. Before rolling back,
check release compatibility with the retained account database and OpenBao data;
changing an image tag does not undo data migrations.

To inspect selected image references without starting anything:

```bash
./compose-wayland.sh config --images
```

Avoid sharing a full `config` dump; it can include sensitive settings.

## Credentials and persistent data

The published OpenBao bootstrap generates the control password and MCP bearer
token only when their stored files are missing or empty. Normal `pull` / `up -d`
keeps the named volumes, so container recreation is not credential regeneration.

```bash
./compose-wayland.sh credentials
```

This displays existing credentials without restarting services. It contains
secrets: keep the output private. The MCP bearer token is distinct from Google's
OAuth tokens and OpenBao's internal AppRole credentials. Internal credential
repair does not mean the client-facing MCP token has been reset.

| Stored state | Named volume(s) in the base stack |
| --- | --- |
| Account metadata | `arqen-data` |
| OpenBao protected store | `openbao-data` |
| OpenBao bootstrap/admin material | `openbao-admin` |
| Control/broker AppRole files | `openbao-control`, `openbao-broker` |
| Control password | `arqen-control-secret` |
| MCP bearer token | `arqen-mcp-secret` |
| Broker socket/runtime files | `arqen-runtime` |

Actual Docker volume names are project-prefixed. Treat account and secret-store
backups as sensitive; keep them private and back up consistent data before
manual storage changes. Do not copy credentials into documentation.

Credentials can change if their files are deleted/emptied or fresh volumes are
created. Moving to another directory/project name can select new volumes and
look like a reset. Never use `down -v`, `docker volume rm`, or volume pruning as
an update procedure: they can destroy accounts, credentials, and protected data.

## Status, shutdown, and troubleshooting

```bash
./compose-wayland.sh ps -a
./compose-wayland.sh stop
```

`stop` stops services while retaining containers and volumes. To resume, use
`up -d`. `./compose-wayland.sh down` removes the stack's containers and networks
but retains named volumes by default; do not add `-v`. Neither is necessary for
an ordinary image update. The wrapper forwards Compose arguments, so destructive
options are not automatically blocked.

Common failures:

- **Wayland socket unavailable:** run from the logged-in desktop session and
  check `WAYLAND_DISPLAY` / `XDG_RUNTIME_DIR`. Do not expose unrelated host
  directories or run as root to bypass the check.
- **Old image still running:** `pull` alone is insufficient. Run `up -d`, check
  selected tags with `config --images`, and inspect the running broker image.
- **OpenBao unhealthy or startup dependency failed:** inspect service status and
  local logs. Do not delete credential volumes or reauthorize accounts as a
  first response. Logs are for private diagnosis, not an unreviewed chat paste.
- **Credential printing fails after startup:** inspect `ps -a`. Services may have
  started even if the final display step failed; do not reset volumes to fix it.
- **New/empty account state:** confirm the same project, directory, and volumes
  are in use before assuming data was lost.

To verify the broker's actual image version/revision without exposing credentials:

```bash
broker_id=$(./compose-wayland.sh ps -q arqen-broker)
test -n "$broker_id"
docker inspect "$broker_id" \
  --format 'status={{.State.Status}} image={{.Config.Image}}'
docker image inspect \
  "$(docker inspect "$broker_id" --format '{{.Image}}')" \
  --format 'version={{index .Config.Labels "org.opencontainers.image.version"}} revision={{index .Config.Labels "org.opencontainers.image.revision"}}'
```

Check these against the intended release's published metadata. Container/image
checks do not prove authentication, Google grants, clipboard behavior, or a
successful Gmail mutation. Any live destructive test needs separate approval
and exact-message readback.

# Quickstart

Run Arqen with Docker Compose and published images. This path does not build
the app from source.

## Before you start

You need:

- Docker with the `docker compose` command.
- A Google account with Gmail.
- Your own Google OAuth Desktop client JSON. Arqen does not include one.

To create the JSON, create or choose a Google Cloud project, enable the Gmail
API, configure the Google Auth Platform, and create an OAuth client for a
**Desktop app**. Follow [Google's Gmail API setup guide](https://developers.google.com/workspace/gmail/api/quickstart/python).
If your OAuth app is in Testing, add your Google account as a test user. Google
expires Gmail refresh tokens for external apps in Testing after seven days, so
you may need to sign in again.

Arqen asks Google for read and modify Gmail permissions. The modify permission
can allow composing and sending email, although Arqen does not offer a send
tool. See [what Arqen stores and what access it requests](security/auth-and-data.md).

## Start Arqen

Clone the repository, then place the downloaded JSON file at the path Compose
expects:

```bash
git clone https://github.com/Operator-Syn/arqen.git
cd arqen
install -d -m 700 .secrets
install -m 600 /path/to/your/desktop-client.json .secrets/google-client-secret.json
```

Start the app:

```bash
docker compose up -d
```

Compose pulls the published Arqen images, initializes the protected local
store, and starts the account screen, credential broker, and MCP server. The
OAuth file is kept private on your computer; Compose stages a readable copy for
the non-root app containers automatically.

To fetch the newest published images later and recreate containers that use
changed images, run:

```bash
docker compose pull
docker compose up -d
```

The root Compose file defaults `ARQEN_DOCKER_IMAGE_TAG` to `latest` and always
checks GHCR when starting services. `pull` only updates the local image cache;
the `up` command applies changed images to running services while preserving
their named volumes. See [Docker image releases](operations/docker-images.md)
for the published version and tag policy.

If you replace the OAuth JSON later, rerun the staging step and restart the
services that use it:

```bash
docker compose up -d --force-recreate arqen-oauth-client-init arqen-broker arqen-control
```

## Sign in and choose an account

1. Open [http://127.0.0.1:7681](http://127.0.0.1:7681).
2. Sign in as `arqen`. Get the generated password with:

   ```bash
   docker compose exec --user 0:0 -T arqen-control cat /run/arqen-control/control-password
   ```

3. In the Arqen screen, press `a`, finish Google sign-in in your browser, then
   press `t` on the account you want your assistant to use.

Arqen keeps the selected account and protected sign-in data in local Docker
volumes. Your assistant cannot choose a different account or access your
Google password or refresh token.

## Connect an MCP client

Use these connection details in an MCP-compatible client:

| Setting | Value |
| --- | --- |
| MCP URL | `http://127.0.0.1:8787/mcp` |
| Authorization header | `Bearer <your MCP token>` |

Get the generated token with:

```bash
docker compose exec --user 0:0 -T arqen-mcp cat /run/arqen-mcp/mcp-bearer-token
```

Keep this token private. Arqen's tools list and read email, manage Gmail labels,
and mark messages read or unread. They always use the account you selected in
Arqen.

## Stop Arqen

```bash
docker compose down
```

This stops the containers and keeps your account data and OpenBao store in
Docker volumes. The services bind to localhost, so they are not reachable from
other devices by default. For remote MCP clients, follow the [reverse proxy
guide](operations/container-and-nginx.md). For sign-in on a remote Docker host,
forward the control and OAuth callback ports over SSH; do not expose the OAuth
callback to the public internet.

## Build from source

This Quickstart uses published images. Make-based workflows for building and
running from source are in the [developer guide](development/local-workflows.md).

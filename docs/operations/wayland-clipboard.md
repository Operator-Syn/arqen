# Optional Wayland clipboard launcher

`compose-wayland.sh` adds desktop clipboard access to the published-image
Compose stack. Run it from a logged-in Wayland desktop session:

```bash
./compose-wayland.sh up -d
```

After successful detached startup, the launcher prints the control UI address,
username, password, MCP URL, and bearer token to your terminal. To display the
credentials again without restarting services:

```bash
./compose-wayland.sh credentials
```

The output contains credentials; keep it private when recording terminal output.
Other Compose commands are supported, for example `./compose-wayland.sh ps`
and `./compose-wayland.sh down`.

The override mounts only the Wayland socket into `arqen-control`. The app
containers run with the desktop UID/GID so they retain access to the private
broker socket. A one-shot container aligns ownership of the existing data and
runtime volumes, and the OAuth staging service creates its cached client file
for the same UID. The launcher stops the app containers before preparing
ownership and starting them again. Named volumes and their contents are kept.

The standard Docker Quickstart continues to use `compose.yaml` independently.
This launcher is an optional desktop workflow.

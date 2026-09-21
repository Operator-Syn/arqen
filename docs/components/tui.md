# Account TUI

**Source:** `src/main.rs`, `src/ui/accounts.rs`, `src/ui/chrome.rs`,
`src/ui/mod.rs`.

The TUI owns human authorization and account choice. Press `t` on a selected
eligible account to persist it as the one MCP target. The card marks the
target, while the details pane reports `MCP target: Selected` or `Not
selected`. Selecting another target replaces the previous subject atomically;
pressing `t` on the target clears it.

An account is eligible only when it is connected, has a recorded exact Gmail
read-only scope, and has a protected credential reference. Existing accounts with
unverified scope evidence cannot be selected until reauthenticated. A target
that later becomes disconnected remains recorded so the operator can see why
MCP access stopped, but the broker will not use it.

The UI continues to own login, reconnect, reauthentication, disconnect
confirmation, modal/error rendering, and responsive pane scrolling. MCP
requests do not mutate these UI states.

## Docker streamed surface

The Docker-native stack serves this TUI through ttyd on `127.0.0.1:7681`. Its
control service forces ttyd's DOM renderer because the pinned ttyd 1.7.7
frontend can measure the initial DOM renderer and then switch to WebGL without a
second fit, leaving unused space until the browser is resized. The workaround
makes the terminal fill the browser viewport on first load. It does not remove
the TUI's small intentional inset around its panels.

Docker mode uses the same native clipboard path as a local TUI session. The
startup script detects Wayland or X11 and mounts only the selected native
clipboard interface into `arqen-control`; the broker, MCP, and OpenBao
containers receive no display access. The `c` action copies the Rust-generated
authorization URL directly, without parsing terminal rows. A headless Docker
host cannot use this action and should use the native/manual OAuth workflow.

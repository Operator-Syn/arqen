# Account TUI

**Source:** `src/main.rs`, `src/ui/accounts.rs`, `src/ui/chrome.rs`,
`src/ui/mod.rs`.

The TUI owns human authorization and account choice. Press `t` on a selected
eligible account to persist it as the one MCP target. The card marks the
target, while the details pane reports `MCP target: Selected` or `Not
selected`. Selecting another target replaces the previous subject atomically;
pressing `t` on the target clears it.

An account is eligible only when it is connected, has a recorded exact Gmail
read-only scope, and has a protected keyring reference. Existing accounts with
unverified scope evidence cannot be selected until reauthenticated. A target
that later becomes disconnected remains recorded so the operator can see why
MCP access stopped, but the broker will not use it.

The UI continues to own login, reconnect, reauthentication, disconnect
confirmation, modal/error rendering, and responsive pane scrolling. MCP
requests do not mutate these UI states.


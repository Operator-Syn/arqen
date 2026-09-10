# Arqen TUI mockup

![Arqen dashboard mockup](google-account-tui-dashboard.png)

This image is the visual reference for the Arqen Ratatui account screen. It is a
design reference only; it does not contain live account data and does not prove
OAuth, keyring, deployment, or runtime behavior.

The original image includes a decorative tagline in the header. The live TUI
intentionally omits that non-actionable copy and keeps only the product
identity, local-session status, and connected-account count.

## Component map

- Header: product identity, local-session status, and connected-account count.
- Account list: keyboard/mouse-selectable identities with one highlighted row.
- Selected account: email identity, connected/disconnected/indeterminate state, provider, confirmed or
  unverified scope evidence, exact granted scopes, and keyring-protected
  credential reference or cleanup status.
- Footer: the same actions are available through keyboard controls and matching
  mouse targets, including disconnect, reconnect, and reauthentication for the
  selected account.
- Dialogs: authorization, redirect entry, and error states reuse the same dark
  surface and focused border treatment.

## Reusable modal reference

![Arqen confirm-quit modal](arqen-confirm-quit-modal.png)

The modal is a reusable Ratatui component. Callers provide a title, body,
semantic tone, actions, and the focused action; the component owns sizing,
padding, responsive action layout, focus styling, and mouse hit-testing.
Components emit action IDs only. They never launch browsers, modify account
data, or access credentials.

Warning modals use amber chrome, destructive actions use pink/red, safe actions
use pastel blue, and supporting copy uses muted text. Actions sit horizontally
when they fit and stack on compact terminals.

Runtime errors from OAuth, storage, browser launch, clipboard providers, and
account refreshes are rendered through the reusable error modal. They are not
written directly to stdout or stderr while the alternate screen is active.
Clipboard writes retain a long-lived owner and can fall back to `wl-copy`,
`xclip`, or `xsel` when available.

## Responsive behavior

Wide terminals use a sidebar and detail panel. Narrow terminals stack the list
above the detail panel. Content must wrap rather than hide required actions.

## Interaction labels

`a` adds an account, `d` disconnects or retries cleanup for the selected account, `l` reconnects a disconnected account, `r` reauthenticates the selected account, `j`/`k` or the arrow keys select an account, `Enter`
inspects or continues, `c` copies an authorization URL, `o` opens it and
starts automatic callback handling, and `Esc` cancels the current modal. From the account screen, `q` opens quit
confirmation; `Enter`/`y` confirms and `Esc`/`n` cancels. Mouse actions have
equivalent keyboard paths. Clicking the selected account's connection badge
opens disconnect confirmation, starts reconnect login, or opens cleanup recovery
according to the recorded state.

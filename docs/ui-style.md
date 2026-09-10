# UI style guide

## Direction

The interface follows a compact terminal-dashboard aesthetic: a near-black
canvas, thin cool-gray borders, dense information panels, and one deliberate
identity accent. Pastel blue is used for focus, actions, and Google identity;
it is not used as a large page background. This keeps the app readable and
visually calm in transparent or dark terminal windows.

## Color tokens

| Token | Hex | Use |
|---|---:|---|
| `background` | `#05080C` | Near-black terminal canvas |
| `surface` | `#090E15` | Tables, cards, modal panels |
| `surface-raised` | `#121D2B` | Selected or emphasized areas |
| `primary` | `#9DD9F7` | Pastel-blue labels, links, actions |
| `primary-strong` | `#6FBEE7` | Focus and active borders |
| `text` | `#DEEBF4` | Primary readable content |
| `muted` | `#8495A3` | Supporting information |
| `border` | `#4A5B6C` | Restrained structural boundaries |
| `success` | `#A5E0C2` | Connected/healthy confirmation |
| `warning` | `#F4D29B` | Caution and recoverable issues |
| `danger` | `#F3A9B8` | Errors and destructive actions |

The canonical Rust definitions live in `src/ui/theme.rs`.

The current visual reference is [the account dashboard mockup](ui-mockups/README.md).

## Composition rules

1. Start with a dark canvas and let borders define structure; do not fill every
   region with a bright color.
2. Use pastel blue as the single primary accent. Reserve green, yellow, and pink
   for semantic status only.
3. Keep panels compact and information-dense. Avoid oversized empty rectangles.
4. Establish hierarchy with a title band, a status band, content panels, and a
   small action footer.
5. Give empty states an explanation and one obvious next action.
6. Keep borders thin and quiet; active dialogs may use `primary-strong`.
7. Keep `text` for essential content and `muted` for secondary information.
8. Label actions consistently with `[ a ]`, `[ d ]`, `[ l ]`, `[ r ]`, `[ c ]`, `[ o ]`, and `[Enter]`.
9. Every mouse action must have a keyboard equivalent.
10. Use ASCII-safe symbols by default. Do not require a particular terminal font.
11. Future screens should import shared tokens from `theme` rather than adding
    one-off RGB values.
12. Keep one scrollbar track plus one breathing cell in every account and
    details pane, including narrow and compact layouts. The track uses `border`;
    the thumb uses `primary-strong` when focused and `muted` otherwise.
13. Use `primary-strong` for the border of the focused pane. This focus cue is
    independent of the scrollbar position and remains visible at the top.
14. On wide layouts, keep footer shortcuts and status notices in separate
    columns with a full-height muted `│` separator. Wrap within each column,
    and keep a shortcut token with its label when a line break is required.
15. In stacked narrow and compact footers, keep shortcut rows left-aligned and
    right-align the status/notice row so it reads as a distinct outcome.
16. Give each scrollable details section a consistent trailing breathing row
    and horizontal separator, including the final section when no content
    follows it.
17. Render label/value details as aligned columns with a muted `│` divider and
    a one-cell value inset; keep the divider stable as labels vary in length.

## Interaction language

- `a` means add/connect an account.
- `d` confirms disconnect for a connected account or retries cleanup for an indeterminate account.
- `l` starts the add-account login flow for a disconnected or indeterminate account.
- `r` reauthenticates the selected account and refreshes its recorded scope grant.
- `c` copies a visible authorization URL.
- `o` opens a visible authorization URL in the default browser.
- Clicking the authorization panel opens its URL when mouse support is enabled.
- `Enter` means continue or submit.
- `Esc` cancels or closes the current modal. From the main account screen it opens quit confirmation.
- `q` opens quit confirmation from the main account screen; `Enter`/`y` confirms and `Esc`/`n` cancels.
- `Tab`/`Shift+Tab` switches focus between the account list and details pane.
- In the account list, `j`/`k`, arrows, `Home`, `End`, and `PageUp`/`PageDown`
  select rows; the selected row is kept visible.
- In the details pane, those keys scroll visual rows. Mouse-wheel scrolling
  focuses and scrolls the pane beneath the pointer.
- The footer must expose `[Tab] focus` and `[Wheel] scroll`. Wide layouts may
  name the active pane; narrow and compact layouts may shorten or wrap labels,
  but must preserve the focus and wheel affordances.
- Scrollbar thumbs are not draggable in v1. Required content is clipped only by
  a scroll offset, never removed from the logical pane model.

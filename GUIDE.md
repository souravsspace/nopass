# Browser extension guide

The nopass browser extension fills logins from the store already on your disk,
and saves new ones into it. It talks to `nopass-host` over native messaging.

**It can create an entry and nothing more.** There is no schema for `edit`,
`rm`, `mv` or `cp`, so nothing already in the store can be rewritten, moved or
destroyed over this wire, and a name that is already taken is refused rather
than overwritten ([ADR-0006](docs/adr/0006-create-only-writes-from-the-extension.md)).
Creating needs only your store's public key, so your passphrase never enters
the browser.

- [Install it](#install-it)
- [Using the popup](#using-the-popup)
- [The inline dropdown](#the-inline-dropdown)
- [What the popup asks the host for](#what-the-popup-asks-the-host-for)
- [The workbench](#the-workbench)
- [Design system](#design-system)
- [Verifying a change](#verifying-a-change)
- [Troubleshooting](#troubleshooting)

## Install it

Build the extension and the host:

```sh
bun install
cd tools/chrome && bun run build     # or tools/firefox
cargo build --release
```

**Chrome / Edge / Brave.** Open `chrome://extensions`, turn on Developer mode,
choose **Load unpacked**, and pick `tools/chrome/.output/chrome-mv3`. Chrome
assigns an extension ID; register the host with it:

```sh
./target/release/nopass-host install --browser chrome --extension-id <id>
```

**Firefox.** Open `about:debugging#/runtime/this-firefox`, choose **Load
Temporary Add-on**, and pick any file inside `tools/firefox/.output/firefox-mv3`.
Firefox identifies an add-on by the ID pinned in its manifest, which is fixed:

```sh
./target/release/nopass-host install --browser firefox \
  --extension-id nopass@souravsspace.github.io
```

Name the browser. `--browser` defaults to `all`, which would register your
Chromium ID for Firefox as well, and Firefox would then refuse the connection.
Valid values: `chrome`, `chromium`, `brave`, `edge`, `firefox`, `all`.

Reopen the popup afterwards. Registration is a one-off per browser profile;
`nopass-host uninstall --browser <name>` undoes it. The Chromium ID changes if
you remove and re-add the unpacked extension.

## Using the popup

The popup is 360 × 556 and holds that shape in every state: the list scrolls,
the header and the footer do not.

### When the store is locked

Type your master passphrase and press **Unlock**. That unlocks your terminal
too, for as long as your configured `cache-ttl` lasts — and the reverse holds,
so warming the agent from a terminal skips this screen entirely. See
[Authentication and the passphrase cache](README.md#authentication-and-the-passphrase-cache).

An unlock has nowhere to live but the agent, so the host starts one — using the
`nopass` sitting beside `nopass-host`, and only then `PATH`. A browser launched
from a Dock or Start-menu icon does **not** inherit your shell's `PATH`, which
is why the neighbour is tried first. If the popup says the agent did not keep
the passphrase, the two binaries have been separated; reinstall, or set
`cache-ttl` and warm the agent from a terminal.

### The pill in the top right

Unlocked, it reads a countdown — `4:28`. That is the agent's remaining lease
reported by the host, not a timer the popup owns; it is read off the wall clock,
so a laptop that sleeps for ten minutes wakes ten minutes closer to locked. At
zero the popup **asks the host** rather than assuming, because something else
may have renewed the lease.

**Click the pill to lock immediately.**

Locked, it is an outlined padlock and the word `Locked`. Lock state is never
carried by colour alone — it survives a colourblind user and a greyscale
screenshot.

### The two lists

| Section | Verb | What it is |
| --- | --- | --- |
| **This page** | `search` | Entries matching on their `url:` line, or on a name that ends in the host. |
| **All items** | `list` | Every name in your store. Names only — `list` cannot return a secret. |

Typing in the search box filters both. The search runs over names, before
anything is decrypted.

### Keyboard and mouse

| Input | What happens |
| --- | --- |
| <kbd>↑</kbd> <kbd>↓</kbd> | Move the cursor. The selected row is the one showing **Fill**. |
| <kbd>↵</kbd> | Fill the selected entry into the page, then close the popup. |
| Click a row | Same as Fill. |
| Copy icon | Puts that entry's password on the clipboard. |
| Hover a row | Sets the same cursor the arrow keys use, so the two can never disagree. |

Fill and copy each go through `get`, one entry at a time. The secret is not
stored, cached or logged on the way through: it exists for the length of the
call and then only inside the field it was written into.

> **The clipboard is not cleared.** The CLI's `nopass show -c` clears after
> `NOPASS_CLIP_TIME` (45 s by default). The extension writes with
> `navigator.clipboard` and clears nothing — the popup is gone by then and
> cannot. Clear it yourself if that matters.

### The other states

| State | What it means |
| --- | --- |
| **Connecting to nopass** | The port is opening. The host exits when the browser drops it, so this is a pause, not a fault. |
| **The nopass host is not reachable** | No manifest names this extension. Run the `nopass-host install` line shown, then reopen the popup. |
| **No store yet** | Run `nopass init` from a terminal. Both screens have a copy button for the command. |

### The toolbar icon

A dot on the icon means unlocked; no dot means locked. The title says which.
There is deliberately **no match count** on the badge — keeping one accurate
would mean a `search` round trip to the host on every navigation.

## The inline dropdown

Focus a login field on a page and nopass offers its matches inline, under the
field. It renders into a closed shadow root, so the page can neither read it nor
restyle it. Picking a row fills the form.

It carries the same palette as the popup, written out by hand: a shadow root
sees neither Tailwind nor an `@font-face` rule. **If you change a colour in
`packages/ui/src/styles/globals.css`, change it in
`packages/extension/lib/dropdown.ts` too.**

## What the popup asks the host for

One mutating verb exists on this wire and it can only create. There is no
schema for `edit`, `rm`, `mv` or `cp`, so nothing already in the store can be
reached.

| Verb | Returns | Used by |
| --- | --- | --- |
| `hello` / `status` | Whether the store exists, and the lock state with its TTL | Every popup open |
| `unlock` / `lock` | The new lock state | The passphrase form, the pill |
| `search` | Matches for an origin — **no secret** | "This page" |
| `list` | Every entry name — **no secret** | "All items" |
| `get` | One entry's secret | Fill, copy, the entry screen |
| `insert` | The name it created — **create only** | "New login" |

The content script's vocabulary is narrower than the popup's on purpose: it may
cause a fill and look up matches, but it may never be handed a secret to read,
it cannot enumerate the store, and it has no `save` — a page's script cannot
put an entry into your store even if it guessed the shape.

## Viewing and adding

Click the **›** on any row to open that entry on its own screen: username,
password, `url:` and TOTP, each with a copy button. The password arrives with
the screen but stays masked until you press **Show**. Clicking the row itself
still fills, which is what the popup is open for most of the time.

The **+** in the header opens **New login**, pre-filled with the current tab's
host as the name and its origin as the website — so the usual case is a
password and a Save. The name is what makes a later fill match: end it in the
site's host, or let the `url:` line do it.

Saving requires the store to be **unlocked**, and refuses a name that already
exists. To change or remove an entry, use `nopass edit` or `nopass rm`.

## The workbench

```sh
cd tools/workbench && bun run dev     # http://localhost:5173
```

The workbench mounts the **real** `Popup` and the real dropdown renderer over a
mock bridge — only the far end is fake. Its built CSS is byte-identical to the
extension's, so what is on screen is what ships. Use it to work on the UI
without a store, a host, or an installed extension.

- Scenario pills across the top: Unlocked, No matches, Locked, No store, Host
  missing, Connecting.
- A Dark toggle, top right.
- A right-hand pane logging every call the popup made to the host, newest first.

If a state can be reached here, it is a state the extension can actually be in.

## Design system

The UI is built on the **Sana** design system: warm-paper neutrals (`#f6f5f4`
canvas, white cards floating on it) and a single structural accent that is
**ink — the same near-black the text is set in, with no hue anywhere**. Nothing
in the chrome competes with the entry being filled. Type is Newsreader
(display), Hanken Grotesk (body) and IBM Plex Mono (secrets and paths).

Everything lives in **`packages/ui/src/styles/globals.css`**. It keeps the
shadcn variable names and repoints their values, so the components in
`packages/ui/src/components` need no edit when the palette moves.

**Light or dark is the system's call, not a setting.** An extension page has
nothing to inherit from, so `followSystemTheme` in
`packages/extension/lib/theme.ts` reads `prefers-color-scheme` and mirrors it
onto a `.dark` class on `<html>`, live — flip your OS theme with the popup open
and it follows. The class, rather than the media query alone, because the
`dark:` utilities in the shared components only fire on a class. The workbench's
Dark toggle drives that same class by hand.

Fonts are **bundled, never fetched**. An extension page may not load a remote
font under MV3's CSP, and a request on popup open would announce that the popup
was opened.

> **Adding a package that imports `globals.css`?** Add an `@source` line for it
> at the top of that file. Every consumer resolves the stylesheet through
> `node_modules`, which Tailwind excludes from automatic source detection — so
> without an entry, Tailwind emits the theme and **not one utility class**, and
> your classes silently do nothing.

## Verifying a change

```sh
bun run lint
bun run check-types
bun run test          # vitest + cargo test
bun run e2e           # playwright, against the workbench
```

Then check that styles actually exist — grep the built CSS for a **utility**,
not a token:

```sh
cd tools/chrome && bun run build
grep -c 'flex-col' .output/chrome-mv3/assets/popup-*.css     # expect 1, not 0
```

A green e2e run proves nothing here: the suite asserts text and roles, never
geometry.

## Troubleshooting

**"The nopass host is not reachable."** The manifest does not name this
extension. Re-run the `nopass-host install` line from
[Install it](#install-it) with the ID currently shown in `chrome://extensions`,
then reopen the popup.

**The popup shows "Connecting to nopass" and stays there.** The host is
starting, or it exited and the next request will restart it. If it persists,
run `nopass-host` by hand to see whether it errors on startup.

**"No store yet" but you have a store.** The host looks in `NOPASS_DIR`, then
`~/.nopass`. A store created under a different `HOME` will not be found.

**Nothing appears under "This page".** Entries match on their `url:` line or on
a name ending in the host. `web/github.com` matches `github.com`; `work/gh` does
not, unless it carries `url: https://github.com`.

**The fields do not fill.** Some sites render their login form late or inside an
iframe. Copy the password from the popup instead.

# nopass — design system

What nopass looks like, and why. One system across the popup, the in-page
panels, the demo pages and the website: same palette, same type, same
restraint.

It is the **Sana** system, adapted. Sana was drawn as a warm-paper, ink-accent
brand in the spirit of Notion's calm and Anthropic's typography; nopass took
its tokens because a password manager has the same job — be legible in two
seconds, then get out of the way. `packages/ui/src/styles/globals.css` is the
implementation; this file is the reasoning.

**Read this before changing a colour, a font, a radius, or writing marketing
copy.** Everything below is a decision that has already been made once.

- [1. What the design is for](#1-what-the-design-is-for)
- [2. Colour](#2-colour)
- [3. Type](#3-type)
- [4. Space, radius, shadow](#4-space-radius-shadow)
- [5. Motion](#5-motion)
- [6. Voice](#6-voice)
- [7. Components](#7-components)
- [8. Surfaces](#8-surfaces)
- [9. Accessibility rules that are not negotiable](#9-accessibility-rules-that-are-not-negotiable)

---

## 1. What the design is for

Someone glances at a 360-pixel popup for two seconds, mid-login, on a laptop
in a bright room. Or a panel appears under a field they are typing into, on a
page they did not design, and they have to tell in one glance that it is not
part of that page.

That is the whole brief. Three consequences, and everything else follows:

1. **Nothing decorative competes with the thing being filled.** No gradients,
   no glass, no illustration, no colour that means nothing.
2. **State is never carried by hue alone.** Locked is an outlined padlock and
   the word *Locked*; unlocked is filled with a countdown. It survives a
   greyscale screenshot and a colourblind reader, because it has to.
3. **Secrets are monospace.** An `l` and a `1` must never be guessed at.

---

## 2. Colour

Warm paper, ink, and almost nothing else.

| Role | Light | Dark | Where |
|---|---|---|---|
| Canvas | `#f6f5f4` | `#1b1b1a` | the page behind everything |
| Card | `#ffffff` | `#272623` | panels, rows, dialogs |
| Foreground | `#191919` | `#edecea` | text |
| Muted foreground | `#615d59` | `#a39e98` | labels, meta, hints |
| Border | `#e6e6e6` | `#33322f` | hairlines, which do the separating |
| **Accent (ink)** | `#191817` | `#edecea` | the primary action, links, focus |

**There is no hue.** Every password manager reaches for teal, green, or neon
on black; the single structural accent here is ink — the same near-black the
text is set in — and it inverts to a light neutral in dark mode rather than
gaining a colour.

Semantic colours exist and are deliberately muted, so they read as meaning
rather than decoration: success `#4f7a5b`, warning `#9a7b3c`, danger
`#b0554e`. They appear in a refusal, a warning about an unprotected key, a
failed unlock. Nowhere else.

Dark mode is never pure black. `#1b1b1a` keeps the warmth and stops the
white-on-black glare that a popup opened at night would otherwise be.

**Rules.** No gradient. No hue as an accent. No colour-only state. A tag
palette exists in the Sana system for decoration; nopass does not use it.

---

## 3. Type

Three families, each with one job.

| Family | Used for | Why |
|---|---|---|
| **Newsreader** (serif) | display headings, hero copy | a literary serif reads as considered rather than as a product launch |
| **Hanken Grotesk** (sans) | all UI and body text | warm humanist grotesque, legible at 11px in a popup |
| **IBM Plex Mono** | secrets, entry names, commands, IDs | disambiguates `l`/`1`/`O`/`0` |

Fonts are **bundled, never fetched**. An extension page may not load a remote
font under MV3's CSP, and a request on popup open would announce that the
popup was opened. The site self-hosts them for the same reason it does not
carry analytics.

Scale, from the Sana tokens: display 64/48/36, headings 28/22/18, body
17/15/13, caption 12. Headings track slightly tight (`-0.011em`); body does
not.

**Rules.** Sentence case everywhere — headings, buttons, labels. Never Title
Case, never ALL CAPS except a small uppercase eyebrow with letter-spacing.

---

## 4. Space, radius, shadow

- **Space**: a 4px base — 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96.
- **Radius**: 6 / 8 / 12 / 20, controls at 8, full pill for switches and tags.
- **Shadow**: three levels, all low-opacity and warm-tinted, used only for
  elevation — a dialog, a toast, the in-page panel. Never a glow.
- **Borders do the separating.** A 1px hairline is the default answer;
  a shadow is what a thing that floats above the page gets.
- **Width**: content sits at a `5xl` maximum. Wider paragraphs read worse and
  a marketing page is still reading.

---

## 5. Motion

Fast and understated: 120ms for a hover, 200ms for a panel, 320ms for
anything larger. `ease-out` going in, `ease-in-out` for movement. Press
scales to 0.97.

**No bounce, no spring overshoot, no parallax, no scroll-jacking.** An
animation is there to explain where something came from; when it draws
attention to itself it has failed.

Every animation must be skippable: honour `prefers-reduced-motion`, and never
put content behind an animation that has to finish before it can be read.

---

## 6. Voice

Calm, direct, quietly warm. This is the same voice the code comments and the
docs are written in.

- **Second person** for product copy: *your store*, *you're all set*.
- **Short sentences.** One idea each. No adverbs, no *simply*, *just*,
  *easily* — if it were simple the sentence would not need to say so.
- **No hype, no exclamation marks, no emoji** in product or marketing copy.
- **Say the true thing plainly**, including the awkward ones: the clipboard is
  not cleared, entry names are not encrypted, losing the store loses passkeys.
  A password manager that oversells its guarantees is worse than a plain one.
- **Empty states state the fact**: "Nothing here yet." Not "Oops!".

Examples that pass: *Reads your local store. It never changes an entry it did
not create.* — *Your store is locked.* — *The nopass host is not reachable.*

---

## 7. Components

`packages/ui` holds the shared set — shadcn-style primitives with the tokens
above: button, input, card, badge, label, separator, spinner, tooltip,
scroll-area, item, field, empty, skeleton. The popup and the website both
build from these, so a change to a control is a change everywhere.

Two surfaces do **not** use them, on purpose: the in-page dropdown
(`lib/dropdown.ts`) and the save prompt (`lib/prompt.ts`) are plain DOM in a
closed shadow root, with the palette written out by hand. A shadow root sees
neither Tailwind nor an `@font-face` rule, and a framework inside a content
script is bytes on every page the user visits.

**If a colour changes in `packages/ui/src/styles/globals.css`, change it in
those two files too.** There is no build step that will tell you.

Icons are [Lucide](https://lucide.dev) in the product, monochrome, inheriting
`currentColor`, at 3.5 or 4 units. Never filled with a hue. (The Sana system
names Hugeicons; nopass already shipped Lucide and one icon set is enough.)

---

## 8. Surfaces

| Surface | Size | Notes |
|---|---|---|
| Popup | 360 × 556, fixed | header and footer fixed, the list scrolls |
| Inline dropdown | width of the field, min 260 | closed shadow root, hairline border, elevation shadow |
| Save prompt | 300 wide, top-right, fixed | closed shadow root, the same palette by hand |
| Website | `max-w-5xl` | the same tokens, more air |
| Demo pages | `34rem` | plain fixtures, styled to match so they look like the product |

---

## 9. Accessibility rules that are not negotiable

1. **Contrast**: body text meets WCAG AA against its surface, in both themes.
2. **Focus is visible**: a soft 3px ink-tinted ring, never `outline: none`
   without a replacement.
3. **State is never colour alone** — see §1.
4. **Every control is reachable by keyboard**, and the popup's list moves with
   the arrow keys and fills on Enter.
5. **Reduced motion is honoured**, everywhere, including the website.
6. **Labels are real labels.** A placeholder is not a label; the in-page
   panels carry `role` and `aria-label` because they live in someone else's
   document.

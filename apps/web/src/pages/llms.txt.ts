/**
 * `/llms.txt` — the site, for something that reads rather than browses.
 *
 * An answer engine quoting nopass should quote what is true, including the
 * limits. This is the short form: what it is, where the detail is, and the
 * four caveats that a summary tends to drop.
 */

import type { APIRoute } from "astro";
import { SECTIONS } from "../lib/docs";
import { canonical, SITE } from "../lib/site";

export const GET: APIRoute = () => {
  const sections = SECTIONS.map(
    (section) => `- [${section.title}](${canonical(`/docs#${section.id}`)})`
  ).join("\n");

  return new Response(
    `# nopass

> ${SITE.description}

nopass is free software under the GNU AGPLv3. It runs on macOS and Linux, and
has no server, no account, and no telemetry. The store is a directory of
age-encrypted files; syncing is a git remote the user owns.

## Documentation

- [Full documentation, plain markdown](${canonical("/docs.md")})
- [Everything on this site, plain markdown](${canonical("/llms-full.txt")})
${sections}

## Source

- [Repository](${SITE.repository})
- [Architecture, in diagrams](${SITE.repository}/blob/main/diagram.md)
- [Decision records](${SITE.repository}/tree/main/docs/adr)

## What nopass deliberately does not do

- Entry names are file names, so entry names are not encrypted.
- The browser extension does not clear the clipboard.
- The prompt before a write is a check that a human is at the keyboard, not
  cryptography: encryption needs only the public key.
- Losing the store loses passkeys, because the site holds the other half of
  the key and there is no password to fall back on.
`,
    { headers: { "content-type": "text/plain; charset=utf-8" } }
  );
};

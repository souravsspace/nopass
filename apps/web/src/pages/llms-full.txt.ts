/**
 * `/llms-full.txt` — the documentation itself, in one plain-text file.
 *
 * Same words as `/docs`, with no navigation, no markup and nothing to parse
 * around. What a model quotes should be what a person reads.
 */

import type { APIRoute } from "astro";
import { asMarkdown } from "../lib/docs";
import { SITE } from "../lib/site";

export const GET: APIRoute = () =>
  new Response(
    `${asMarkdown()}\n---\n\nSource: ${SITE.repository}\nLicence: GNU AGPLv3\n`,
    { headers: { "content-type": "text/plain; charset=utf-8" } }
  );

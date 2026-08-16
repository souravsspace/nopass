/**
 * `/docs.md` — the documentation as markdown.
 *
 * The same sections the page renders. A reader who prefers a terminal, and a
 * crawler that would rather not unwrap a layout, get the same words.
 */

import type { APIRoute } from "astro";
import { asMarkdown } from "../lib/docs";

export const GET: APIRoute = () =>
  new Response(asMarkdown(), {
    headers: { "content-type": "text/markdown; charset=utf-8" },
  });

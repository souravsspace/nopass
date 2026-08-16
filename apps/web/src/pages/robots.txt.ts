/**
 * `/robots.txt`.
 *
 * Everything here is public documentation for free software; there is nothing
 * to keep out of an index, and answer engines are named explicitly rather
 * than left to infer that they are welcome.
 */

import type { APIRoute } from "astro";
import { canonical } from "../lib/site";

export const GET: APIRoute = () =>
  new Response(
    `User-agent: *
Allow: /

Sitemap: ${canonical("/sitemap-index.xml")}
`,
    { headers: { "content-type": "text/plain; charset=utf-8" } }
  );

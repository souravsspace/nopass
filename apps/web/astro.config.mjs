// @ts-check
import react from "@astrojs/react";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";

import { SITE } from "./src/lib/site.ts";

/**
 * Static output, and no adapter.
 *
 * There is nothing on this site that needs a server: no forms, no accounts,
 * no analytics. A directory of files is also the honest shape for a product
 * whose whole claim is that nothing leaves your machine.
 *
 * React is here for the few pieces that have to move — the install tabs, the
 * copy buttons, the animated sections — and is loaded per island, so a reader
 * who never touches one downloads none of it.
 */
export default defineConfig({
  integrations: [react(), sitemap()],
  site: SITE.url,
  vite: { plugins: [tailwindcss()] },
});

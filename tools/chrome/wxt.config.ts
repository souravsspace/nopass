import { fileURLToPath } from "node:url";
import { SHARED_CONFIG, SHARED_MANIFEST } from "@nopass/extension/wxt-shared";
import { defineConfig } from "wxt";

/**
 * The Chromium build: Chrome, Chromium, Brave and Edge all take this one.
 *
 * MV3 here means a service worker background, and an extension ID derived
 * from a `key` in the manifest. No key is committed: an unpacked development
 * build gets whatever ID the browser assigns, and `nopass-host install
 * --extension-id <id>` takes that ID. A published build gets its stable ID
 * from the Web Store. Set `NOPASS_CHROME_KEY` to pin one locally.
 */
export default defineConfig({
  ...SHARED_CONFIG,
  manifest: {
    ...SHARED_MANIFEST,
    // biome-ignore lint/style/noProcessEnv: this is a node build script
    ...(process.env.NOPASS_CHROME_KEY
      ? { key: process.env.NOPASS_CHROME_KEY }
      : {}),
  },
  outDir: fileURLToPath(new URL(".output", import.meta.url)),
});

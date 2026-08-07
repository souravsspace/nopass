import { fileURLToPath } from "node:url";
import { SHARED_CONFIG, SHARED_MANIFEST } from "@nopass/extension/wxt-shared";
import { defineConfig } from "wxt";

/**
 * The Firefox build.
 *
 * MV3 on Gecko differs in two ways that matter here. The background is an
 * event page rather than a service worker, and the add-on must carry an
 * explicit ID: `browser_specific_settings.gecko.id` is mandatory for signing
 * through AMO, and the native messaging manifest names that exact ID, so it
 * has to be stable and known before either can be installed.
 */
export const GECKO_ID = "nopass@souravsspace.github.io";

export default defineConfig({
  ...SHARED_CONFIG,
  manifest: {
    ...SHARED_MANIFEST,
    browser_specific_settings: {
      gecko: {
        // Required for new add-ons on AMO since November 2025. nopass sends
        // nothing anywhere: the store is local and there is no telemetry.
        data_collection_permissions: { required: ["none"] },
        id: GECKO_ID,
        // MV3 event pages landed well before this; nothing older is worth
        // supporting for a native messaging extension.
        strict_min_version: "115.0",
      },
    },
  },
  // WXT still defaults Gecko to MV2. nopass wants the MV3 event page.
  manifestVersion: 3,
  outDir: fileURLToPath(new URL(".output", import.meta.url)),
});

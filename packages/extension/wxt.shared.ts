/**
 * Everything `tools/chrome` and `tools/firefox` agree on.
 *
 * The targets exist to hold what genuinely differs between the two browsers —
 * the background style, the pinned extension ID, packaging — and nothing
 * else. If something lands here that only one browser needs, it is in the
 * wrong file (ADR-0003).
 */

import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";

/** This package, which is where every entrypoint actually lives. */
export const SOURCE_DIR = fileURLToPath(new URL(".", import.meta.url));

export const SHARED_MANIFEST = {
  action: { default_title: "nopass" },
  description:
    "Fill logins from your local nopass store, and save new ones to it.",
  host_permissions: ["http://*/*", "https://*/*"],
  name: "nopass",
  permissions: [
    // Reaching the host at all.
    "nativeMessaging",
    // Knowing which tab the popup was opened over, and messaging it.
    "tabs",
    // Copying a password from the popup.
    "clipboardWrite",
  ],
};

export const SHARED_CONFIG = {
  modules: ["@wxt-dev/module-react"],
  // `root`, not `srcDir`: WXT emits entrypoint HTML at a path relative to the
  // root, and a source directory outside it produces `../../` names the
  // bundler refuses. Pointing the root at this package keeps every entrypoint
  // inside it; each target only redirects the output back to its own folder.
  root: SOURCE_DIR,
  // The popup imports Tailwind's entry from @nopass/ui.
  vite: () => ({ plugins: [tailwindcss()] }),
};

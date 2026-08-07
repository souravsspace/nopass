import { defineConfig, devices } from "@playwright/test";

/**
 * The workbench is the e2e target for the extension's UI.
 *
 * It serves the real `Popup` and the real dropdown renderer over a mock
 * bridge, so a browser can drive the whole interaction — unlock, search,
 * fill, lock — without a store, a host, or an installed extension. What sits
 * below the bridge is covered by the Rust host's own tests and by the shared
 * protocol fixtures.
 */
export default defineConfig({
  forbidOnly: Boolean(process.env.CI),
  fullyParallel: true,
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  reporter: process.env.CI ? "github" : "list",
  retries: process.env.CI ? 2 : 0,
  testDir: "./e2e",
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "on-first-retry",
  },
  webServer: {
    command: "bun run build && bun run preview",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    url: "http://127.0.0.1:4173",
  },
});

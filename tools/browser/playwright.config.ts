import { defineConfig } from "@playwright/test";

/**
 * The extension, in a real browser, against the real host.
 *
 * Everything below the extension is genuine here: `nopass-host` is the built
 * binary, the store is a real age-encrypted tree written by the real CLI, and
 * the pages are the demo fixtures served over http. What that buys is the
 * class of bug the unit tests cannot see — a request whose shape the host
 * refuses, a dropdown that opens and is closed again by a stray timer, a fill
 * that lands in the wrong box.
 *
 * One worker: each test builds its own store and its own browser profile, and
 * a shared native messaging host is not something to contend over.
 */
export default defineConfig({
  forbidOnly: Boolean(process.env.CI),
  fullyParallel: false,
  reporter: process.env.CI ? "github" : "list",
  retries: 0,
  testDir: "./e2e",
  timeout: 60_000,
  use: { trace: "on-first-retry" },
  webServer: {
    command: "bun run ../demo/serve.ts",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
    url: "http://127.0.0.1:8790/",
  },
  workers: 1,
});

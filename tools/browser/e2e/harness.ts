/**
 * A browser with the extension in it, talking to a real nopass.
 *
 * Three real things and no stubs: the unpacked extension out of
 * `tools/chrome/.output`, the `nopass-host` binary the browser launches over
 * native messaging, and an age-encrypted store the `nopass` CLI wrote into a
 * temporary directory. The only concession is the identity, which is created
 * with `--no-passphrase` so the host reads the store as unlocked — the agent
 * and its TTL have their own tests, and an e2e that had to type a passphrase
 * into a terminal it does not own would be testing the wrong thing.
 *
 * `NOPASS_DIR` and `NOPASS_CONFIG` are set on the browser process, which
 * passes its environment to the host it spawns; that is what keeps each test
 * inside its own store.
 */

import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  type BrowserContext,
  test as base,
  chromium,
  type Page,
} from "@playwright/test";

/** One entry per file, and the extension the store writes. */
const ENTRY_FILE = /\.np$/;

const REPO = resolve(import.meta.dirname, "../../..");
const EXTENSION = join(REPO, "tools/chrome/.output/chrome-mv3");
const CLI = join(REPO, "target/debug/nopass");
const HOST = join(REPO, "target/debug/nopass-host");

/** Where the demo fixtures are served. Matches `playwright.config.ts`. */
export const DEMO = "http://127.0.0.1:8790";

/**
 * The unpacked extension's ID.
 *
 * Chromium derives it from the absolute path of the directory it is loaded
 * from, so it is stable for a given checkout but not across machines. It is
 * read back from the running browser rather than guessed.
 */
async function extensionId(context: BrowserContext): Promise<string> {
  const worker =
    context.serviceWorkers()[0] ??
    (await context.waitForEvent("serviceworker"));
  return new URL(worker.url()).host;
}

export interface Store {
  /** The directory `NOPASS_DIR` points at. */
  readonly dir: string;
  /** Write an entry, the way a user would from a terminal. */
  insert: (
    name: string,
    kind: string,
    secret: string,
    fields: string[]
  ) => void;
  /** Every entry name currently in the store. */
  list: () => string[];
  /** The decrypted body of an entry, for asserting what actually landed. */
  show: (name: string) => string;
}

function makeStore(root: string): Store {
  const dir = join(root, "store");
  const config = join(root, "config");
  const identity = join(root, "identity.txt");
  const env = {
    ...process.env,
    NOPASS_CONFIG: config,
    NOPASS_DIR: dir,
  };

  const run = (args: string[]) =>
    execFileSync(CLI, args, { encoding: "utf8", env });

  run(["keygen", "--identity", identity, "--no-passphrase"]);
  run(["init"]);

  return {
    dir,
    insert(name, kind, secret, fields) {
      const flags = fields.flatMap((field) => ["--field", field]);
      execFileSync(CLI, ["insert", "-e", "--type", kind, ...flags, name], {
        encoding: "utf8",
        env,
        input: `${secret}\n`,
      });
    },
    list() {
      // Read off disk rather than out of `nopass ls`, whose output is a tree
      // for a person to look at. What is on disk is also what is being
      // asserted about: one `.np` file per entry.
      const walk = (at: string, prefix: string): string[] =>
        readdirSync(join(dir, at), { withFileTypes: true }).flatMap((found) => {
          if (found.isDirectory()) {
            return walk(join(at, found.name), `${prefix}${found.name}/`);
          }
          return found.name.endsWith(".np")
            ? [`${prefix}${found.name.replace(ENTRY_FILE, "")}`]
            : [];
        });
      // Sorted by name, which is the order the host lists them in.
      return walk(".", "").sort((left, right) => left.localeCompare(right));
    },
    show(name) {
      return run(["show", name]);
    },
  };
}

/** Register the built host for this profile, so the browser can launch it. */
function registerHost(profile: string, id: string): void {
  const dir = join(profile, "NativeMessagingHosts");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "com.nopass.host.json"),
    JSON.stringify({
      allowed_origins: [`chrome-extension://${id}/`],
      description: "nopass native messaging host",
      name: "com.nopass.host",
      path: HOST,
      type: "stdio",
    })
  );
}

/**
 * How the browser is started.
 *
 * `CHROMIUM_PATH` is for a machine whose Playwright browsers were installed
 * out of band — this container is one — and is left off entirely when it is
 * unset, rather than passed as undefined.
 */
function launch(env: NodeJS.ProcessEnv) {
  const path = process.env.CHROMIUM_PATH;
  return {
    args: [
      `--disable-extensions-except=${EXTENSION}`,
      `--load-extension=${EXTENSION}`,
      "--headless=new",
    ],
    env,
    ...(path ? { executablePath: path } : {}),
  };
}

interface Fixtures {
  /** A page with the extension loaded and the host registered. */
  page: Page;
  /** The store this test's browser is looking at. */
  store: Store;
}

export const test = base.extend<Fixtures>({
  page: async ({ store }, use) => {
    const profile = mkdtempSync(join(tmpdir(), "nopass-profile-"));
    const env = {
      ...process.env,
      NOPASS_CONFIG: join(store.dir, "..", "config"),
      NOPASS_DIR: store.dir,
    };

    // Loaded twice on purpose: the ID is only knowable once a browser has the
    // extension, and the native messaging manifest has to name that ID before
    // the extension asks for the host. The first launch is thrown away.
    const first = await chromium.launchPersistentContext(profile, launch(env));
    const id = await extensionId(first);
    await first.close();
    registerHost(profile, id);

    const context = await chromium.launchPersistentContext(
      profile,
      launch(env)
    );

    try {
      await use(await context.newPage());
    } finally {
      await context.close();
      rmSync(profile, { force: true, recursive: true });
    }
  },
  // biome-ignore lint/correctness/noEmptyPattern: Playwright's fixture signature
  store: async ({}, use) => {
    const root = mkdtempSync(join(tmpdir(), "nopass-e2e-"));
    try {
      await use(makeStore(root));
    } finally {
      rmSync(root, { force: true, recursive: true });
    }
  },
});

/*
 * Re-exported so a spec imports `test` and `expect` from one place — the one
 * that knows about the store and the browser it hands them.
 */
// biome-ignore lint/performance/noBarrelFile: two names, not a barrel
export { expect } from "@playwright/test";

/**
 * A vault that does not exist, for a host that is not running.
 *
 * The workbench renders the real `Popup` with a real `Bridge`; only the far
 * end is fake. That is the point: if a state can be reached here, it is a
 * state the extension can actually be in.
 */

import type { Bridge } from "@nopass/extension/lib/bridge";
import type { SessionState } from "@nopass/extension/lib/session";
import type { Match, Secret } from "@nopass/protocol";

export const ENTRIES: Secret[] = [
  {
    name: "web/github.com",
    password: "9x!Kd2pQvr4TmZ",
    totp: "otpauth://totp/github?secret=JBSWY3DPEHPK3PXP",
    url: "https://github.com",
    username: "sana@example.com",
  },
  {
    name: "web/github.com-work",
    password: "Lp7#eR1wYq0NbA",
    url: "https://github.com",
    username: "sana@work.example",
  },
  { name: "web/mail.github.com", password: "Tz3$hV8mCk5JdE" },
  {
    name: "personal/news.ycombinator.com",
    password: "Qw2@nX6bFs9RgU",
    username: "sana",
  },
];

/** The states worth being able to look at without reproducing them by hand. */
export type Scenario =
  | "unlocked"
  | "locked"
  | "no-store"
  | "unavailable"
  | "connecting"
  | "empty";

export const SCENARIOS: { id: Scenario; label: string; note: string }[] = [
  { id: "unlocked", label: "Unlocked", note: "Entries for the current site" },
  { id: "empty", label: "No matches", note: "Unlocked, nothing for this site" },
  { id: "locked", label: "Locked", note: "Passphrase needed" },
  { id: "no-store", label: "No store", note: "Before nopass init" },
  {
    id: "unavailable",
    label: "Host missing",
    note: "Not registered with the browser",
  },
  {
    id: "connecting",
    label: "Connecting",
    note: "The first moment the popup opens",
  },
];

const never = new Promise<never>(() => undefined);

function stateFor(scenario: Scenario): SessionState {
  switch (scenario) {
    case "unlocked":
    case "empty":
      return { expiresIn: 268, status: "unlocked" };
    case "locked":
      return { status: "locked" };
    case "no-store":
      return { status: "no-store" };
    case "unavailable":
      return {
        message: "Specified native messaging host not found.",
        status: "unavailable",
      };
    default:
      return { status: "connecting" };
  }
}

/** Records what the popup asked for, so the workbench can show it. */
export type Log = (line: string) => void;

export function mockBridge(scenario: Scenario, log: Log): Bridge {
  let state = stateFor(scenario);

  return {
    currentOrigin() {
      return Promise.resolve("https://github.com");
    },
    fill(entry: string) {
      log(`fill(${entry})`);
      return Promise.resolve();
    },
    lock() {
      log("lock()");
      state = { status: "locked" };
      return Promise.resolve(state);
    },
    reveal(entry: string) {
      log(`reveal(${entry})`);
      const found = ENTRIES.find((candidate) => candidate.name === entry);
      return found
        ? Promise.resolve(found)
        : Promise.reject(new Error(`${entry} is not in the store`));
    },
    search(origin: string) {
      log(`search(${origin})`);
      if (scenario === "empty") {
        return Promise.resolve([]);
      }
      const matches: Match[] = ENTRIES.filter((entry) =>
        entry.url?.includes("github")
      ).map(({ name, username, url }) => ({ name, url, username }));
      return Promise.resolve(matches);
    },
    session() {
      if (scenario === "connecting") {
        return never;
      }
      return Promise.resolve(state);
    },
    unlock(passphrase: string) {
      log(`unlock(${"•".repeat(passphrase.length)})`);
      state = { expiresIn: 300, status: "unlocked" };
      return Promise.resolve(state);
    },
  };
}

/**
 * A vault that does not exist, for a host that is not running.
 *
 * The workbench renders the real `Popup` with a real `Bridge`; only the far
 * end is fake. That is the point: if a state can be reached here, it is a
 * state the extension can actually be in.
 */

import { type Bridge, BridgeError } from "@nopass/extension/lib/bridge";
import type { SessionState } from "@nopass/extension/lib/session";
import type { Field, Item, Kind, Match, Secret } from "@nopass/protocol";

/** `key: value` lines, minus the ones nobody filled in. */
function fields(pairs: [string, string | undefined][]): Field[] {
  return pairs
    .filter((pair): pair is [string, string] => Boolean(pair[1]))
    .map(([key, value]) => ({ key, value }));
}

export const ENTRIES: Secret[] = [
  {
    fields: fields([
      ["username", "sana@example.com"],
      ["url", "https://github.com"],
      ["totp", "otpauth://totp/github?secret=JBSWY3DPEHPK3PXP"],
    ]),
    kind: "login",
    name: "web/github.com",
    secret: "9x!Kd2pQvr4TmZ",
  },
  {
    fields: fields([
      ["username", "sana@work.example"],
      ["url", "https://github.com"],
    ]),
    kind: "login",
    name: "web/github.com-work",
    secret: "Lp7#eR1wYq0NbA",
  },
  {
    fields: [],
    kind: "login",
    name: "web/mail.github.com",
    secret: "Tz3$hV8mCk5JdE",
  },
  {
    fields: fields([["username", "sana"]]),
    kind: "login",
    name: "personal/news.ycombinator.com",
    secret: "Qw2@nX6bFs9RgU",
  },
  {
    fields: fields([
      ["cardholder", "Sana Qureshi"],
      ["exp-month", "04"],
      ["exp-year", "2029"],
      ["brand", "Visa"],
    ]),
    kind: "card",
    name: "cards/visa",
    secret: "4111111111114242",
  },
  {
    fields: fields([
      ["given-name", "Sana"],
      ["family-name", "Qureshi"],
      ["email", "sana@example.com"],
      ["street", "12 Example Road"],
      ["city", "Dhaka"],
      ["country", "Bangladesh"],
    ]),
    kind: "identity",
    name: "me/home",
    secret: "",
  },
];

/** The row an entry shows in a list: a name, a kind, and never a secret. */
function itemFor(entry: Secret): Item {
  const value = (key: string) =>
    entry.fields.find((field) => field.key === key)?.value;

  switch (entry.kind) {
    case "card": {
      const tail = entry.secret.slice(-4);
      const brand = value("brand");
      return {
        hint: brand ? `${brand} •••• ${tail}` : `•••• ${tail}`,
        kind: entry.kind,
        name: entry.name,
      };
    }
    case "identity": {
      const name = [value("given-name"), value("family-name")]
        .filter(Boolean)
        .join(" ");
      return { hint: name, kind: entry.kind, name: entry.name };
    }
    default: {
      const username = value("username");
      return {
        kind: entry.kind,
        name: entry.name,
        ...(username ? { hint: username } : {}),
      };
    }
  }
}

/**
 * The one passphrase this fake store opens for. A refusal is reachable here
 * the same way it is in the browser — by typing the wrong thing — rather than
 * through a scenario button the extension does not have.
 */
export const PASSPHRASE = "correct horse battery staple";

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
  {
    id: "locked",
    label: "Locked",
    note: `Opens for "${PASSPHRASE}"; anything else is refused`,
  },
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
    generate(length: number, symbols: boolean) {
      log(`generate(${length}, symbols: ${symbols})`);
      // The real one is the host's CSPRNG. This one only has to be long
      // enough, and different every time, to see the screen behave.
      const alphabet = symbols
        ? "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%^&*-_"
        : "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
      const made = Array.from(
        { length },
        () => alphabet[Math.floor(Math.random() * alphabet.length)]
      ).join("");
      return Promise.resolve(made);
    },
    items(kinds?: Kind[]) {
      log(`items(${kinds?.join(", ") ?? "all"})`);
      const wanted = ENTRIES.filter(
        (entry) => !kinds || kinds.includes(entry.kind)
      );
      return Promise.resolve(wanted.map(itemFor));
    },
    list() {
      log("list()");
      return Promise.resolve(ENTRIES.map((entry) => entry.name));
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

    save(draft) {
      log(`save(${draft.entry}, ${draft.kind ?? "login"})`);
      // The refusal that matters is the one the real host makes: a name
      // already taken is never overwritten (ADR-0006).
      if (ENTRIES.some((entry) => entry.name === draft.entry)) {
        return Promise.reject(
          new BridgeError("exists", `${draft.entry} is already in the store`)
        );
      }
      ENTRIES.push({
        fields: draft.fields ?? [],
        kind: draft.kind ?? "login",
        name: draft.entry,
        secret: draft.secret ?? "",
      });
      return Promise.resolve(draft.entry);
    },
    search(origin: string) {
      log(`search(${origin})`);
      if (scenario === "empty") {
        return Promise.resolve([]);
      }
      const matches: Match[] = ENTRIES.filter((entry) =>
        entry.fields.some(
          (field) => field.key === "url" && field.value.includes("github")
        )
      ).map((entry) => ({
        kind: entry.kind,
        name: entry.name,
        url: entry.fields.find((field) => field.key === "url")?.value,
        username: entry.fields.find((field) => field.key === "username")?.value,
      }));
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
      if (passphrase !== PASSPHRASE) {
        return Promise.reject(
          new BridgeError("locked", "that passphrase did not open the store")
        );
      }
      state = { expiresIn: 300, status: "unlocked" };
      return Promise.resolve(state);
    },

    update(patch) {
      log(`update(${patch.entry})`);
      const found = ENTRIES.find((entry) => entry.name === patch.entry);
      if (!found) {
        return Promise.reject(
          new BridgeError("not_found", `${patch.entry} is not in the store`)
        );
      }
      if (patch.secret !== undefined) {
        found.secret = patch.secret;
      }
      for (const field of patch.fields ?? []) {
        const at = found.fields.findIndex((have) => have.key === field.key);
        // An empty value is how the host is told to clear a field.
        if (!field.value) {
          if (at !== -1) {
            found.fields.splice(at, 1);
          }
        } else if (at === -1) {
          found.fields.push(field);
        } else {
          found.fields[at] = field;
        }
      }
      return Promise.resolve(patch.entry);
    },
  };
}

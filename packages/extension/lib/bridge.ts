/**
 * What the popup is allowed to do, as one injectable interface.
 *
 * The real implementation talks to the background over `runtime.sendMessage`.
 * `tools/workbench` supplies a mock one, so the popup rendered there is the
 * same component with the same props, not a lookalike.
 */

import type { Item, Kind, Match, Secret } from "@nopass/protocol";
import type { Draft, Patch } from "./messaging";
import type { SessionState } from "./session";

export interface Bridge {
  /** The origin of the tab the popup was opened over, if it has one. */
  currentOrigin: () => Promise<string | null>;
  /** Fill the active tab with an entry. */
  fill: (entry: string) => Promise<void>;
  /**
   * A fresh random password, made by the host.
   *
   * The same generator the CLI uses, drawing on the same OS entropy: the
   * browser's idea of randomness never comes into it, and a password that is
   * never typed is one the page cannot have watched being typed.
   */
  generate: (length: number, symbols: boolean) => Promise<string>;
  /** Rows for the kinds asked for: a name, a kind, and a hint that is never
   * a secret. Absent kinds asks for the whole store. */
  items: (kinds?: Kind[]) => Promise<Item[]>;
  /** Every entry name in the store. Names only — never a secret. */
  list: () => Promise<string[]>;
  lock: () => Promise<SessionState>;
  /** One entry's secret, for showing or copying in the popup. */
  reveal: (entry: string) => Promise<Secret>;
  /**
   * Create one entry. It can only ever create: a name already taken comes
   * back as an `exists` refusal rather than replacing anything (ADR-0006).
   */
  save: (draft: Draft) => Promise<string>;
  /** Entries offered for a page origin. Empty for a non-web tab. */
  search: (origin: string) => Promise<Match[]>;
  session: () => Promise<SessionState>;
  unlock: (passphrase: string) => Promise<SessionState>;
  /**
   * Change an entry that is already there, one named field at a time.
   *
   * The screen that calls this asks the user to confirm the replacement
   * first; nothing here can check that, which is why it is a rule about the
   * UI and not about the wire (ADR-0009).
   */
  update: (patch: Patch) => Promise<string>;
}

export class BridgeError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}

/**
 * What the popup is allowed to do, as one injectable interface.
 *
 * The real implementation talks to the background over `runtime.sendMessage`.
 * `tools/workbench` supplies a mock one, so the popup rendered there is the
 * same component with the same props, not a lookalike.
 */

import type { Match, Secret } from "@nopass/protocol";
import type { Draft } from "./messaging";
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
  /** Every entry name in the store. Names only — never a secret. */
  list: () => Promise<string[]>;
  lock: () => Promise<SessionState>;
  /** One entry's secret, for showing or copying in the popup. */
  reveal: (entry: string) => Promise<Secret>;
  /**
   * Create one entry. The only call here that changes the store, and it can
   * only ever create: a name already taken comes back as an `exists` refusal
   * rather than replacing anything (ADR-0006).
   */
  save: (draft: Draft) => Promise<string>;
  /** Entries offered for a page origin. Empty for a non-web tab. */
  search: (origin: string) => Promise<Match[]>;
  session: () => Promise<SessionState>;
  unlock: (passphrase: string) => Promise<SessionState>;
}

export class BridgeError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}
